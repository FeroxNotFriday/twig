use crate::model::Config;
use crate::parse::Expr;
use crate::tree::TreeNode;

fn has_active_filter(cfg: &Config) -> bool {
    match &cfg.expr {
        None => false,
        Some(Expr::And(items)) => !items.is_empty(),
        Some(_) => true,
    }
}

fn cmp_ci(a: &str, b: &str) -> std::cmp::Ordering {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let n = ab.len().min(bb.len());
    for i in 0..n {
        let ca = ab[i].to_ascii_lowercase();
        let cb = bb[i].to_ascii_lowercase();
        match ca.cmp(&cb) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
    }
    ab.len().cmp(&bb.len())
}

pub fn prune_and_aggregate(node: &mut TreeNode, cfg: &Config) -> bool {
    if !node.is_dir {
        node.total_size = node.size;
        node.file_count = 1;
        node.dir_count = 0;
        return true;
    }

    let mut keep_children = Vec::new();
    let mut total_bytes = 0;
    let mut files = 0;
    let mut dirs = 0;

    for mut child in std::mem::take(&mut node.children) {
        if prune_and_aggregate(&mut child, cfg) {
            total_bytes += child.total_size;
            if child.is_dir {
                dirs += 1 + child.dir_count;
                files += child.file_count;
            } else {
                files += 1;
            }
            keep_children.push(child);
        }
    }

    node.children = keep_children;
    node.total_size = total_bytes;
    node.file_count = files;
    node.dir_count = dirs;
    node.index.clear();

    node.children.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => cmp_ci(&a.name, &b.name),
    });

    let should_prune = has_active_filter(cfg) && !cfg.dirs_only && cfg.depth.is_none();
    !(node.children.is_empty() && should_prune)
}
