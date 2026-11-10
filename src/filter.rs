use crate::model::Config;
use crate::parse::{Expr, Leaf, Sign};
use globset::{GlobBuilder, GlobMatcher};
use regex::Regex;
use std::borrow::Cow;
use std::fs::Metadata;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub enum CompiledKind {
    Size(Sign, u64),
    Time(Sign, i64),
    Lines(Sign, usize),
    Chars(Sign, usize),
    Ext(Vec<String>),
    Glob(GlobMatcher),
    Regex(Regex),
    Substring(String),
}

#[derive(Clone, Debug)]
pub enum CompiledExpr {
    And(Vec<CompiledExpr>),
    Or(Vec<CompiledExpr>),
    Not(Box<CompiledExpr>),
    Leaf(CompiledKind),
}

fn compile(expr: &Expr) -> CompiledExpr {
    match expr {
        Expr::And(items) => CompiledExpr::And(items.iter().map(compile).collect()),
        Expr::Or(items) => CompiledExpr::Or(items.iter().map(compile).collect()),
        Expr::Not(inner) => CompiledExpr::Not(Box::new(compile(inner))),
        Expr::Leaf(leaf) => CompiledExpr::Leaf(compile_leaf(leaf)),
    }
}

fn compile_leaf(leaf: &Leaf) -> CompiledKind {
    match leaf {
        Leaf::Size(s, v) => CompiledKind::Size(*s, *v),
        Leaf::Time(s, v) => CompiledKind::Time(*s, *v),
        Leaf::Lines(s, v) => CompiledKind::Lines(*s, *v),
        Leaf::Chars(s, v) => CompiledKind::Chars(*s, *v),
        Leaf::Ext(list) => CompiledKind::Ext(list.iter().map(|s| s.to_ascii_lowercase()).collect()),
        Leaf::Glob(pattern) => {
            let norm = pattern.replace('\\', "/");
            match GlobBuilder::new(&norm).literal_separator(false).build() {
                Ok(g) => CompiledKind::Glob(g.compile_matcher()),
                Err(_) => CompiledKind::Substring(pattern.clone()),
            }
        }
        Leaf::Regex(pattern) => match Regex::new(pattern) {
            Ok(r) => CompiledKind::Regex(r),
            Err(_) => CompiledKind::Substring(pattern.clone()),
        },
        Leaf::Substring(s) => CompiledKind::Substring(s.clone()),
    }
}

fn expr_needs_rel(e: &CompiledExpr) -> bool {
    match e {
        CompiledExpr::And(v) | CompiledExpr::Or(v) => v.iter().any(expr_needs_rel),
        CompiledExpr::Not(x) => expr_needs_rel(x),
        CompiledExpr::Leaf(k) => matches!(
            k,
            CompiledKind::Glob(_) | CompiledKind::Regex(_) | CompiledKind::Substring(_)
        ),
    }
}

pub struct Matcher {
    pub expr: CompiledExpr,
    pub needs_rel_path: bool,
}

impl Matcher {
    pub fn new(cfg: &Config) -> Self {
        let expr = cfg
            .expr
            .as_ref()
            .map(compile)
            .unwrap_or(CompiledExpr::And(Vec::new()));
        let needs_rel_path = expr_needs_rel(&expr);
        Self {
            expr,
            needs_rel_path,
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(&self.expr, CompiledExpr::And(v) if v.is_empty())
    }

    pub fn matches(
        &self,
        path: &Path,
        rel_path: &Path,
        is_dir: bool,
        size: u64,
        meta: &Metadata,
        now: SystemTime,
    ) -> bool {
        let rel_cow: Cow<str> = if self.needs_rel_path {
            let s = rel_path.to_string_lossy();
            if s.contains('\\') {
                Cow::Owned(s.replace('\\', "/"))
            } else {
                s
            }
        } else {
            Cow::Borrowed("")
        };

        let ctx = Ctx {
            path,
            rel_path: &rel_cow,
            file_name: path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
            size,
            meta,
            is_dir,
            now,
        };
        !matches!(evaluate(&self.expr, &ctx), Some(false))
    }
}

struct Ctx<'a> {
    path: &'a Path,
    rel_path: &'a str,
    file_name: &'a str,
    size: u64,
    meta: &'a Metadata,
    is_dir: bool,
    now: SystemTime,
}

impl<'a> Ctx<'a> {
    fn age_secs(&self) -> Option<i64> {
        let modified = self.meta.modified().ok()?;
        let dur = self.now.duration_since(modified).ok()?;
        Some(dur.as_secs() as i64)
    }
}

fn and3(a: Option<bool>, b: Option<bool>) -> Option<bool> {
    match (a, b) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (None, _) | (_, None) => None,
        _ => Some(true),
    }
}

fn or3(a: Option<bool>, b: Option<bool>) -> Option<bool> {
    match (a, b) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (None, _) | (_, None) => None,
        _ => Some(false),
    }
}

fn evaluate(expr: &CompiledExpr, ctx: &Ctx) -> Option<bool> {
    match expr {
        CompiledExpr::And(items) => {
            let mut r = Some(true);
            for e in items {
                r = and3(r, evaluate(e, ctx));
                if r == Some(false) {
                    return r;
                }
            }
            r
        }
        CompiledExpr::Or(items) => {
            let mut r = Some(false);
            for e in items {
                r = or3(r, evaluate(e, ctx));
                if r == Some(true) {
                    return r;
                }
            }
            r
        }
        CompiledExpr::Not(inner) => evaluate(inner, ctx).map(|b| !b),
        CompiledExpr::Leaf(k) => eval_leaf(k, ctx),
    }
}

fn eval_leaf(leaf: &CompiledKind, ctx: &Ctx) -> Option<bool> {
    match leaf {
        CompiledKind::Size(sign, target) => {
            if ctx.is_dir {
                return None;
            }
            Some(match sign {
                Sign::Greater => ctx.size > *target,
                Sign::Less => ctx.size < *target,
                Sign::AtLeast => ctx.size >= *target,
                Sign::Exact => ctx.size == *target,
            })
        }
        CompiledKind::Time(sign, secs) => {
            if ctx.is_dir {
                return None;
            }
            let age = ctx.age_secs()?;
            Some(match sign {
                Sign::Greater => age > *secs,
                Sign::Less => age < *secs,
                Sign::AtLeast => age >= *secs,
                Sign::Exact => age == *secs,
            })
        }
        CompiledKind::Lines(sign, target) => {
            if ctx.is_dir {
                return None;
            }
            let n = count_file_lines(ctx.path, ctx.size).unwrap_or(0);
            Some(match sign {
                Sign::Greater => n > *target,
                Sign::Less => n < *target,
                Sign::AtLeast => n >= *target,
                Sign::Exact => n == *target,
            })
        }
        CompiledKind::Chars(sign, target) => {
            if ctx.is_dir {
                return None;
            }
            let n = count_file_chars(ctx.path, ctx.size).unwrap_or(0);
            Some(match sign {
                Sign::Greater => n > *target,
                Sign::Less => n < *target,
                Sign::AtLeast => n >= *target,
                Sign::Exact => n == *target,
            })
        }
        CompiledKind::Ext(list) => {
            let fname = ctx.file_name;
            let fname_no_dot = fname.strip_prefix('.').unwrap_or(fname);
            let matched_ext = ctx
                .path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| list.iter().any(|t| t.eq_ignore_ascii_case(e)))
                .unwrap_or(false);
            let matched_name = list.iter().any(|t| {
                let t_no_dot = t.strip_prefix('.').unwrap_or(t);
                fname_no_dot.eq_ignore_ascii_case(t_no_dot)
            });
            if matched_ext || matched_name {
                return Some(true);
            }
            if ctx.is_dir {
                return None;
            }
            Some(false)
        }
        CompiledKind::Glob(m) => {
            let v = m.is_match(ctx.file_name) || m.is_match(ctx.rel_path);
            if v {
                Some(true)
            } else if ctx.is_dir {
                None
            } else {
                Some(false)
            }
        }
        CompiledKind::Regex(r) => {
            let v = r.is_match(ctx.file_name) || r.is_match(ctx.rel_path);
            if v {
                Some(true)
            } else if ctx.is_dir {
                None
            } else {
                Some(false)
            }
        }
        CompiledKind::Substring(sub) => {
            let v = ctx.file_name.contains(sub.as_str()) || ctx.rel_path.contains(sub.as_str());
            if v {
                Some(true)
            } else if ctx.is_dir {
                None
            } else {
                Some(false)
            }
        }
    }
}

const BINARY_EXT: &[&str] = &[
    "exe", "dll", "so", "dylib", "a", "lib", "rlib", "rmeta", "obj", "o", "pdb", "d", "bin", "png",
    "jpg", "jpeg", "gif", "bmp", "ico", "webp", "tiff", "zip", "tar", "gz", "7z", "rar", "bz2",
    "xz", "zst", "mp3", "mp4", "wav", "flac", "avi", "mkv", "mov", "webm", "pdf", "doc", "docx",
    "xls", "xlsx", "ppt", "pptx", "iso", "dat", "db", "sqlite", "ttf", "otf", "woff", "woff2",
];

fn is_binary_ext(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => BINARY_EXT.iter().any(|b| b.eq_ignore_ascii_case(ext)),
        None => false,
    }
}

const LOC_MAX_FILE: u64 = 4 * 1024 * 1024;

pub fn count_file_lines(path: &Path, size: u64) -> Option<usize> {
    if is_binary_ext(path) || size > LOC_MAX_FILE {
        return None;
    }
    use memchr::memchr_iter;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; 16384];
    let mut count = 0;
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => count += memchr_iter(b'\n', &buf[..n]).count(),
            Err(_) => return None,
        }
    }
    Some(count)
}

pub fn count_file_chars(path: &Path, size: u64) -> Option<usize> {
    if is_binary_ext(path) || size > LOC_MAX_FILE {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; 16384];
    let mut count = 0;
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => count += buf[..n].iter().filter(|&&b| b & 0xC0 != 0x80).count(),
            Err(_) => return None,
        }
    }
    Some(count)
}
