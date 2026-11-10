#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Greater,
    Less,
    AtLeast,
    Exact,
}

#[derive(Clone, Debug)]
pub enum Leaf {
    Size(Sign, u64),
    Time(Sign, i64),
    Lines(Sign, usize),
    Chars(Sign, usize),
    Ext(Vec<String>),
    Glob(String),
    Regex(String),
    Substring(String),
}

#[derive(Clone, Debug)]
pub enum Expr {
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Not(Box<Expr>),
    Leaf(Leaf),
}

#[derive(Clone, Debug)]
enum Token {
    Plus,
    Minus,
    Eq,
    Percent,
    OrOp,
    Dots,
    LParen,
    RParen,
    Qty(Sign, f64, Unit),
    Name(String),
}

#[derive(Clone, Copy, Debug)]
enum Unit {
    Bytes,
    KiB,
    MiB,
    GiB,
    TiB,
    PiB,
    Nanos,
    Micros,
    Millis,
    Secs,
    Mins,
    Hours,
    Days,
    Weeks,
    Months,
    Years,
    Lines,
    Chars,
}

fn unit_from_str(s: &str) -> Option<Unit> {
    Some(match s.to_lowercase().as_str() {
        "b" => Unit::Bytes,
        "kb" | "k" => Unit::KiB,
        "mb" => Unit::MiB,
        "gb" | "g" => Unit::GiB,
        "tb" | "t" => Unit::TiB,
        "pb" | "p" => Unit::PiB,
        "ns" => Unit::Nanos,
        "us" | "µs" => Unit::Micros,
        "ms" => Unit::Millis,
        "s" => Unit::Secs,
        "m" => Unit::Mins,
        "h" => Unit::Hours,
        "d" => Unit::Days,
        "w" => Unit::Weeks,
        "mo" => Unit::Months,
        "y" => Unit::Years,
        "l" => Unit::Lines,
        "c" => Unit::Chars,
        _ => return None,
    })
}

fn is_unit_token(s: &str) -> bool {
    unit_from_str(s).is_some()
}

fn is_pure_number(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn is_signed_pure_number(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix('+').or_else(|| s.strip_prefix('-')) {
        is_pure_number(rest)
    } else {
        false
    }
}

fn normalize(raw: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let t = raw[i].trim();
        if t.is_empty() {
            i += 1;
            continue;
        }

        if t == "+" || t == "-" || t == "=" {
            if i + 1 < raw.len() && is_pure_number(raw[i + 1].trim()) {
                if i + 2 < raw.len() && is_unit_token(raw[i + 2].trim()) {
                    out.push(format!("{}{}{}", t, raw[i + 1].trim(), raw[i + 2].trim()));
                    i += 3;
                } else {
                    out.push(format!("{}{}", t, raw[i + 1].trim()));
                    i += 2;
                }
                continue;
            }
            out.push(t.to_string());
            i += 1;
            continue;
        }

        if is_signed_pure_number(t) {
            if i + 1 < raw.len() && is_unit_token(raw[i + 1].trim()) {
                out.push(format!("{}{}", t, raw[i + 1].trim()));
                i += 2;
            } else {
                out.push(t.to_string());
                i += 1;
            }
            continue;
        }

        if is_pure_number(t) && i + 1 < raw.len() && is_unit_token(raw[i + 1].trim()) {
            out.push(format!("{}{}", t, raw[i + 1].trim()));
            i += 2;
            continue;
        }

        out.push(t.to_string());
        i += 1;
    }
    out
}

fn split_percent_prefix(raw: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(raw.len());
    for s in raw {
        if s.len() > 1 && s.starts_with('%') {
            out.push("%".to_string());
            out.push(s[1..].to_string());
        } else {
            out.push(s);
        }
    }
    out
}

fn classify(s: &str) -> Token {
    match s {
        "+" => Token::Plus,
        "-" => Token::Minus,
        "=" => Token::Eq,
        "%" => Token::Percent,
        "$" => Token::OrOp,
        "..." | "…" => Token::Dots,
        _ => {
            if let Some(tok) = try_parse_quantity(s) {
                return tok;
            }
            Token::Name(s.to_string())
        }
    }
}

fn try_parse_quantity(s: &str) -> Option<Token> {
    let (sign, rest) = if let Some(r) = s.strip_prefix('+') {
        (Sign::Greater, r)
    } else if let Some(r) = s.strip_prefix('-') {
        (Sign::Less, r)
    } else if let Some(r) = s.strip_prefix('=') {
        (Sign::Exact, r)
    } else {
        (Sign::AtLeast, s)
    };

    if rest.is_empty() {
        return None;
    }

    let idx = rest.find(|c: char| !c.is_ascii_digit() && c != '.')?;
    let (num_str, unit_str) = rest.split_at(idx);
    if num_str.is_empty() {
        return None;
    }
    let value: f64 = num_str.parse().ok()?;
    let unit = unit_from_str(unit_str)?;
    Some(Token::Qty(sign, value, unit))
}

fn resolve_dots(tokens: Vec<Token>) -> Result<Vec<Token>, String> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut depth: usize = 0;
    let mut expect_operand = true;

    for t in tokens {
        match t {
            Token::Dots => {
                if expect_operand {
                    out.push(Token::LParen);
                    depth += 1;
                } else if depth > 0 {
                    out.push(Token::RParen);
                    depth -= 1;
                    expect_operand = false;
                } else {
                    return Err("unmatched '...' — there is no open group to close".into());
                }
            }
            Token::OrOp => {
                out.push(t);
                expect_operand = true;
            }
            Token::Percent => {
                out.push(t);
                expect_operand = true;
            }
            Token::Plus | Token::Minus | Token::Eq => {
                out.push(t);
                expect_operand = true;
            }
            Token::Qty(..) | Token::Name(..) => {
                out.push(t);
                expect_operand = false;
            }
            Token::LParen | Token::RParen => {
                out.push(t);
            }
        }
    }

    while depth > 0 {
        out.push(Token::RParen);
        depth -= 1;
    }

    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut items = vec![self.parse_and()?];
        while matches!(self.peek(), Some(Token::OrOp)) {
            self.pos += 1;
            items.push(self.parse_and()?);
        }
        Ok(if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Expr::Or(items)
        })
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut items = Vec::new();
        loop {
            match self.peek() {
                None | Some(Token::RParen) | Some(Token::OrOp) => break,
                _ => items.push(self.parse_unary()?),
            }
        }
        if items.is_empty() {
            return Err("expected an expression".into());
        }
        Ok(if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Expr::And(items)
        })
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if matches!(self.peek(), Some(Token::Percent)) {
            self.pos += 1;
            let inner = self.parse_unary()?;
            return Ok(Expr::Not(Box::new(inner)));
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Result<Expr, String> {
        let tok = self
            .tokens
            .get(self.pos)
            .cloned()
            .ok_or_else(|| "unexpected end of expression".to_string())?;
        self.pos += 1;
        match tok {
            Token::LParen => {
                let inner = self.parse_or()?;
                match self.tokens.get(self.pos) {
                    Some(Token::RParen) => {
                        self.pos += 1;
                        Ok(inner)
                    }
                    _ => Err("expected closing '...'".into()),
                }
            }
            Token::Qty(sign, value, unit) => Ok(Expr::Leaf(qty_to_leaf(sign, value, unit))),
            Token::Name(n) => Ok(Expr::Leaf(name_to_leaf(&n))),
            Token::Plus => Err("'+' must be followed by a number+unit (e.g. +10mb)".into()),
            Token::Minus => Err("'-' must be followed by a number+unit (e.g. -10mb)".into()),
            Token::Eq => Err("'=' must be followed by a number+unit (e.g. =10mb)".into()),
            Token::Percent => Err("'%' must be followed by an expression".into()),
            Token::OrOp => Err("'$' must be between expressions".into()),
            Token::Dots => unreachable!(),
            Token::RParen => Err("unexpected closing '...'".into()),
        }
    }
}

fn qty_to_leaf(sign: Sign, value: f64, unit: Unit) -> Leaf {
    match unit {
        Unit::Bytes => Leaf::Size(sign, value as u64),
        Unit::KiB => Leaf::Size(sign, (value * 1024.0) as u64),
        Unit::MiB => Leaf::Size(sign, (value * 1024.0 * 1024.0) as u64),
        Unit::GiB => Leaf::Size(sign, (value * 1024.0 * 1024.0 * 1024.0) as u64),
        Unit::TiB => Leaf::Size(sign, (value * 1024f64.powi(4)) as u64),
        Unit::PiB => Leaf::Size(sign, (value * 1024f64.powi(5)) as u64),
        Unit::Nanos => Leaf::Time(sign, (value / 1e9).max(0.0) as i64),
        Unit::Micros => Leaf::Time(sign, (value / 1e6).max(0.0) as i64),
        Unit::Millis => Leaf::Time(sign, (value / 1e3).max(0.0) as i64),
        Unit::Secs => Leaf::Time(sign, value.max(0.0) as i64),
        Unit::Mins => Leaf::Time(sign, (value * 60.0).max(0.0) as i64),
        Unit::Hours => Leaf::Time(sign, (value * 3600.0).max(0.0) as i64),
        Unit::Days => Leaf::Time(sign, (value * 86400.0).max(0.0) as i64),
        Unit::Weeks => Leaf::Time(sign, (value * 604800.0).max(0.0) as i64),
        Unit::Months => Leaf::Time(sign, (value * 2592000.0).max(0.0) as i64),
        Unit::Years => Leaf::Time(sign, (value * 31536000.0).max(0.0) as i64),
        Unit::Lines => Leaf::Lines(sign, value.max(0.0) as usize),
        Unit::Chars => Leaf::Chars(sign, value.max(0.0) as usize),
    }
}

fn name_to_leaf(s: &str) -> Leaf {
    let s = s.trim();

    if s.len() > 2 && s.starts_with('/') && s.ends_with('/') {
        return Leaf::Regex(s[1..s.len() - 1].to_string());
    }

    if s.contains('*') || s.contains('?') || (s.contains('[') && s.contains(']')) {
        return Leaf::Glob(s.to_string());
    }

    let stripped = if let Some(r) = s.strip_prefix("./") {
        r
    } else if let Some(r) = s.strip_prefix(".\\") {
        r
    } else {
        s
    };

    let stripped = stripped.trim_end_matches(['/', '\\']);

    if stripped.is_empty() {
        return Leaf::Substring(s.to_string());
    }

    let looks_like_ext = stripped.starts_with('.')
        || (!stripped.contains('.')
            && !stripped.contains('/')
            && !stripped.contains('\\')
            && stripped
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ',')
            && stripped.chars().next().is_some_and(|c| c.is_alphabetic()));

    if looks_like_ext {
        let exts: Vec<String> = stripped
            .trim_start_matches('.')
            .split(',')
            .map(|x| x.trim().trim_start_matches('.').to_lowercase())
            .filter(|x| {
                !x.is_empty()
                    && x.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            })
            .collect();
        if !exts.is_empty() {
            return Leaf::Ext(exts);
        }
    }

    Leaf::Substring(stripped.to_string())
}

pub fn parse_expression(raw: &[String]) -> Result<Expr, String> {
    if raw.is_empty() {
        return Ok(Expr::And(Vec::new()));
    }
    for tok in raw {
        let t = tok.trim();
        if t == "@" {
            return Err("'@' is not a valid operator. Use '$' for OR".into());
        }
        if t.eq_ignore_ascii_case("or") {
            return Err("'or' is not a valid operator. Use '$' for OR".into());
        }
    }
    let norm = normalize(raw);
    let norm = split_percent_prefix(norm);
    let raw_tokens: Vec<Token> = norm.iter().map(|s| classify(s)).collect();
    let tokens = resolve_dots(raw_tokens)?;
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.parse_or()?;
    if parser.pos < parser.tokens.len() {
        return Err("unexpected tokens after expression".into());
    }
    Ok(expr)
}