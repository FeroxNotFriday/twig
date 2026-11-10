#![allow(clippy::collapsible_if)]

use clap::Parser;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use twig::model::*;
use twig::parse;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser, Debug)]
#[command(
    name = "twig",
    version,
    about = "Fast, modern directory tree with a filter expression language."
)]
pub struct Cli {
    #[arg(value_name = "EXPR", help = "Path and/or filter expression", allow_hyphen_values = true)]
    pub args: Vec<String>,

    #[arg(short = 'd', value_name = "N", help = "Max tree depth")]
    pub depth: Option<usize>,

    #[arg(short = 'n', value_name = "N", help = "Max files per directory")]
    pub limit: Option<usize>,

    #[arg(long = "gitignore", help = "Respect .gitignore")]
    pub respect_gitignore: bool,

    #[arg(short = 's', help = "Show file sizes")]
    pub show_bytes: bool,

    #[arg(short = 'z', help = "Show recursive folder sizes")]
    pub show_disk: bool,

    #[arg(short = 'l', help = "Show lines of code")]
    pub show_loc: bool,

    #[arg(short = 't', help = "Show modification times")]
    pub show_dates: bool,

    #[arg(short = 'c', help = "Show file/dir counters")]
    pub show_count: bool,

    #[arg(short = 'a', help = "Show hidden files")]
    pub show_all: bool,

    #[arg(short = 'D', help = "Directories only")]
    pub dirs_only: bool,

    #[arg(short = 'x', help = "Stay on same filesystem")]
    pub same_fs: bool,

    #[arg(short = 'j', help = "JSON output")]
    pub json: bool,

    #[arg(short = 'r', help = "Raw output (no ANSI colors)")]
    pub raw: bool,
}

fn preprocess_args(raw: Vec<String>) -> Vec<String> {
    const BOOL_SHORT: &[char] = &['s', 'z', 'l', 't', 'c', 'a', 'D', 'x', 'j', 'r', 'h', 'V'];
    const LONG_FLAGS: &[&str] = &["--gitignore", "--help", "--version"];
    const SHORT_WITH_VALUE: &[&str] = &["-d", "-n"];

    let mut flags: Vec<String> = Vec::with_capacity(raw.len());
    let mut positionals: Vec<String> = Vec::new();
    let mut after_dd = false;
    let mut seen_bool: HashSet<char> = HashSet::new();
    let mut with_value: Vec<(String, String)> = Vec::new();

    let mut iter = raw.into_iter();
    if let Some(prog) = iter.next() {
        flags.push(prog);
    }

    while let Some(arg) = iter.next() {
        if after_dd {
            positionals.push(arg);
            continue;
        }
        if arg == "--" {
            after_dd = true;
            continue;
        }
        if LONG_FLAGS.contains(&arg.as_str()) {
            if !flags.iter().any(|f| f == &arg) {
                flags.push(arg);
            }
            continue;
        }
        if SHORT_WITH_VALUE.contains(&arg.as_str()) {
            let val = iter.next().unwrap_or_default();
            with_value.retain(|(k, _)| k != &arg);
            with_value.push((arg, val));
            continue;
        }
        let is_short_bool = arg.len() >= 2
            && arg.starts_with('-')
            && !arg.starts_with("--")
            && arg[1..].chars().all(|c| BOOL_SHORT.contains(&c));
        if is_short_bool {
            for c in arg[1..].chars() {
                if seen_bool.insert(c) {
                    flags.push(format!("-{}", c));
                }
            }
            continue;
        }
        positionals.push(arg);
    }

    for (k, v) in with_value {
        flags.push(k);
        flags.push(v);
    }
    flags.extend(positionals);
    flags
}

fn resolve_path(s: &str) -> Option<PathBuf> {
    if s.len() > 2 && s.chars().all(|c| c == '.') {
        return None;
    }
    if s.starts_with('%') || s.starts_with('$') {
        return None;
    }
    if Path::new(s).exists() {
        return Some(PathBuf::from(s));
    }
    #[cfg(windows)]
    {
        if s.starts_with('/') && !s.starts_with("//") && s.len() > 1 {
            let alt = s.trim_start_matches('/');
            let alt_no_trailing = alt.trim_end_matches('/');
            if !alt_no_trailing.is_empty() {
                if Path::new(alt_no_trailing).exists() {
                    return Some(PathBuf::from(alt_no_trailing));
                }
                if Path::new(alt).exists() {
                    return Some(PathBuf::from(alt_no_trailing));
                }
            }
        }
    }
    None
}

fn main() {
    let cli = Cli::parse_from(preprocess_args(std::env::args().collect()));

    let mut config = Config {
        respect_gitignore: cli.respect_gitignore,
        show_bytes: cli.show_bytes,
        show_disk: cli.show_disk,
        show_loc: cli.show_loc,
        show_dates: cli.show_dates,
        show_count: cli.show_count,
        show_all: cli.show_all,
        dirs_only: cli.dirs_only,
        same_fs: cli.same_fs,
        json: cli.json,
        raw: cli.raw,
        depth: cli.depth,
        limit: cli.limit,
        ..Default::default()
    };

    let mut target_path: Option<PathBuf> = None;
    let mut filter_tokens: Vec<String> = Vec::new();
    let mut path_slot_used = false;

    for raw in &cli.args {
        let s = raw.trim();
        if s.is_empty() {
            continue;
        }
        if !path_slot_used {
            if let Some(p) = resolve_path(s) {
                target_path = Some(p);
                path_slot_used = true;
                continue;
            }
        }
        path_slot_used = true;
        filter_tokens.push(s.to_string());
    }

    match parse::parse_expression(&filter_tokens) {
        Ok(expr) => config.expr = Some(expr),
        Err(msg) => eprintln!("\x1b[1;33mtwig warning:\x1b[0m {msg} (ignored)"),
    }

    config.path = target_path.unwrap_or_else(|| PathBuf::from("."));

    let matcher = Arc::new(twig::filter::Matcher::new(&config));

    if let Some(mut root) = twig::crawler::crawl(&config, &matcher) {
        let _ = twig::prune::prune_and_aggregate(&mut root, &config);
        twig::render::render_tree(&root, &config);
    }
}