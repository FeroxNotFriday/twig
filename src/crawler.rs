use crate::filter::Matcher;
use crate::model::*;
use crate::tree::TreeNode;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

fn effective_size(meta: &std::fs::Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.blocks() == 0 && meta.len() > 0 {
            return 0;
        }
    }
    meta.len()
}

pub fn crawl(cfg: &Config, matcher: &Arc<Matcher>) -> Option<TreeNode> {
    let root_path = &cfg.path;
    let root_meta = match std::fs::metadata(root_path) {
        Ok(m) => m,
        Err(_) => {
            eprintln!(
                "\x1b[1;31mtwig error:\x1b[0m path '{}' not found or inaccessible",
                root_path.display()
            );
            return None;
        }
    };

    let root_name = root_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(".")
        .to_string();

    let now = SystemTime::now();

    let mut root_node = if root_meta.is_dir() {
        TreeNode::new_dir(root_name, root_path.clone())
    } else {
        let size = effective_size(&root_meta);
        let mut f = TreeNode::new_file(root_name, root_path.clone(), size);
        if cfg.show_loc {
            f.line_count = crate::filter::count_file_lines(root_path, size);
        }
        if cfg.show_dates {
            if let Ok(m) = root_meta.modified() {
                if let Ok(d) = now.duration_since(m) {
                    f.modified_ago = Some(format_ago(d));
                }
            }
        }
        return Some(f);
    };

    let has_filter = !matcher.is_empty();

    let mut builder = WalkBuilder::new(root_path);
    builder.hidden(!cfg.show_all);
    builder.git_ignore(cfg.respect_gitignore);
    builder.git_global(cfg.respect_gitignore);
    builder.git_exclude(cfg.respect_gitignore);
    builder.parents(cfg.respect_gitignore);

    if let Some(depth) = cfg.depth {
        builder.max_depth(Some(depth));
    }

    if cfg.same_fs {
        builder.same_file_system(true);
    }

    if has_filter {
        let filter_root = root_path.clone();
        let filter_matcher = Arc::clone(matcher);
        builder.filter_entry(move |entry| {
            let path = entry.path();
            if path == filter_root.as_path() {
                return true;
            }
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => return false,
            };
            let rel = match path.strip_prefix(&filter_root) {
                Ok(r) => r,
                Err(_) => return false,
            };
            filter_matcher.matches(path, rel, meta.is_dir(), effective_size(&meta), &meta, now)
        });
    }

    for result in builder.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if path == root_path {
            continue;
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if cfg.dirs_only && !meta.is_dir() {
            continue;
        }

        let rel_path = match path.strip_prefix(root_path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let is_dir = meta.is_dir();
        let size = effective_size(&meta);
        let modified = if !is_dir && cfg.show_dates {
            meta.modified().ok()
        } else {
            None
        };

        insert_into_tree(
            &mut root_node,
            rel_path,
            path.to_path_buf(),
            is_dir,
            size,
            modified,
            cfg,
        );
    }

    Some(root_node)
}

fn insert_into_tree(
    root: &mut TreeNode,
    rel_path: &Path,
    full_path: PathBuf,
    is_dir: bool,
    size: u64,
    modified: Option<SystemTime>,
    cfg: &Config,
) {
    let mut current = root;
    let mut components = rel_path.components().peekable();

    while let Some(comp) = components.next() {
        let is_last = components.peek().is_none();
        let comp_os = comp.as_os_str();

        if is_last {
            let name = comp_os.to_string_lossy().into_owned();
            if is_dir {
                if !current.index.contains_key(&name) {
                    let idx = current.children.len();
                    current
                        .children
                        .push(TreeNode::new_dir(name.clone(), full_path.clone()));
                    current.index.insert(name, idx);
                }
            } else {
                let mut f = TreeNode::new_file(name, full_path.clone(), size);
                if cfg.show_loc {
                    f.line_count = crate::filter::count_file_lines(&full_path, size);
                }
                if let Some(mtime) = modified {
                    if let Ok(d) = SystemTime::now().duration_since(mtime) {
                        f.modified_ago = Some(format_ago(d));
                    }
                }
                current.children.push(f);
            }
        } else {
            let name = comp_os.to_string_lossy().into_owned();
            let next_idx = match current.index.get(&name) {
                Some(&idx) => idx,
                None => {
                    let idx = current.children.len();
                    let sub_dir = TreeNode::new_dir(name.clone(), current.path.join(comp_os));
                    current.children.push(sub_dir);
                    current.index.insert(name, idx);
                    idx
                }
            };
            current = &mut current.children[next_idx];
        }
    }
}

fn format_ago(d: Duration) -> String {
    let s = d.as_secs();
    if s < 60 {
        format!("{}s ago", s)
    } else if s < 3600 {
        format!("{}m ago", s / 60)
    } else if s < 86400 {
        format!("{}h ago", s / 3600)
    } else {
        format!("{}d ago", s / 86400)
    }
}