use crate::parse::Expr;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub path: PathBuf,
    pub expr: Option<Expr>,
    pub depth: Option<usize>,
    pub limit: Option<usize>,
    pub show_bytes: bool,
    pub show_disk: bool,
    pub show_loc: bool,
    pub show_dates: bool,
    pub show_count: bool,
    pub show_all: bool,
    pub dirs_only: bool,
    pub same_fs: bool,
    pub json: bool,
    pub raw: bool,
    pub respect_gitignore: bool,
}
