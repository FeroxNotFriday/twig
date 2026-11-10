use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub total_size: u64,
    pub line_count: Option<usize>,
    pub modified_ago: Option<String>,
    pub file_count: usize,
    pub dir_count: usize,
    pub children: Vec<TreeNode>,
    #[serde(skip)]
    pub index: HashMap<String, usize>,
}

impl TreeNode {
    pub fn new_file(name: String, path: PathBuf, size: u64) -> Self {
        Self {
            name,
            path,
            is_dir: false,
            size,
            total_size: size,
            line_count: None,
            modified_ago: None,
            file_count: 0,
            dir_count: 0,
            children: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn new_dir(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            is_dir: true,
            size: 0,
            total_size: 0,
            line_count: None,
            modified_ago: None,
            file_count: 0,
            dir_count: 0,
            children: Vec::new(),
            index: HashMap::new(),
        }
    }
}
