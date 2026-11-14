use crate::model::Config;
use crate::tree::TreeNode;
use humansize::{DECIMAL, format_size};
use std::io::{self, BufWriter, Write};

pub fn render_tree(root: &TreeNode, cfg: &Config) {
    let stdout = io::stdout();
    let mut writer = BufWriter::with_capacity(64 * 1024, stdout.lock());

    if cfg.json {
        let _ = serde_json::to_writer_pretty(&mut writer, root);
        let _ = writeln!(writer);
        let _ = writer.flush();
        return;
    }

    let raw = cfg.raw;
    let root_metas = collect_metas(root, cfg);

    if raw {
        let _ = write!(writer, "{}", root.name);
    } else if root.is_dir {
        let _ = write!(writer, "\x1b[1;34m{}\x1b[0m", root.name);
    } else {
        let _ = write!(writer, "\x1b[0m{}\x1b[0m", root.name);
    }
    write_metadata(&root_metas, raw, &mut writer);
    let _ = writeln!(writer);

    render_children(&root.children, "", cfg, &mut writer);

    let summary = format!(
        "[✓] {} files, {} directories | {}",
        root.file_count,
        root.dir_count,
        format_size(root.total_size, DECIMAL)
    );
    if raw {
        let _ = writeln!(writer, "\n{}", summary);
    } else {
        let _ = writeln!(writer, "\n\x1b[90m{}\x1b[0m", summary);
    }
    let _ = writer.flush();
}

fn collect_metas(node: &TreeNode, cfg: &Config) -> Vec<(String, &'static str)> {
    let mut metas: Vec<(String, &'static str)> = Vec::new();

    if !node.is_dir && cfg.show_bytes {
        let color = if node.size > 10 * 1024 * 1024 {
            "\x1b[1;31m"
        } else if node.size > 100 * 1024 {
            "\x1b[33m"
        } else {
            "\x1b[90m"
        };
        metas.push((format_size(node.size, DECIMAL), color));
    }

    if node.is_dir && cfg.show_disk {
        metas.push((format_size(node.total_size, DECIMAL), "\x1b[1;33m"));
    }

    if let Some(loc) = node.line_count {
        metas.push((format!("{} lines", loc), "\x1b[35m"));
    }

    if let Some(ref date_str) = node.modified_ago {
        metas.push((date_str.clone(), "\x1b[32m"));
    }

    if node.is_dir && cfg.show_count {
        metas.push((
            format!("[{}f, {}d]", node.file_count, node.dir_count),
            "\x1b[90m",
        ));
    }

    metas
}

fn write_metadata<W: Write>(metas: &[(String, &'static str)], raw: bool, writer: &mut W) {
    if metas.is_empty() {
        return;
    }
    if raw {
        let _ = write!(writer, " (");
        for (i, (text, _)) in metas.iter().enumerate() {
            if i > 0 {
                let _ = write!(writer, ", ");
            }
            let _ = write!(writer, "{}", text);
        }
        let _ = write!(writer, ")");
    } else {
        let _ = write!(writer, " \x1b[90m(");
        for (i, (text, color)) in metas.iter().enumerate() {
            if i > 0 {
                let _ = write!(writer, ", ");
            }
            let _ = write!(writer, "{}{}\x1b[0m", color, text);
        }
        let _ = write!(writer, "\x1b[90m)\x1b[0m");
    }
}

fn render_children<W: Write>(children: &[TreeNode], prefix: &str, cfg: &Config, writer: &mut W) {
    let count = children.len();
    let limit = cfg.limit.unwrap_or(usize::MAX);
    let shown = count.min(limit);
    let has_more = count > limit;
    let total_items = shown + if has_more { 1 } else { 0 };
    let raw = cfg.raw;

    for (i, child) in children.iter().take(shown).enumerate() {
        let is_last = i == total_items - 1;
        let branch = if is_last { "└── " } else { "├── " };
        let next_prefix = if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{}│   ", prefix)
        };

        format_node_to(child, cfg, prefix, branch, writer);

        if child.is_dir && !child.children.is_empty() {
            render_children(&child.children, &next_prefix, cfg, writer);
        }
    }

    if has_more {
        let remaining = count - shown;
        let plural = if remaining == 1 { "item" } else { "items" };
        if raw {
            let _ = writeln!(
                writer,
                "{}└── ... (and {} more {})",
                prefix, remaining, plural
            );
        } else {
            let _ = writeln!(
                writer,
                "{}\x1b[90m└── ... (and {} more {})\x1b[0m",
                prefix, remaining, plural
            );
        }
    }
}

fn format_node_to<W: Write>(
    node: &TreeNode,
    cfg: &Config,
    prefix: &str,
    branch: &str,
    writer: &mut W,
) {
    let raw = cfg.raw;
    let _ = write!(writer, "{}{}", prefix, branch);

    if node.is_dir {
        if raw {
            let _ = write!(writer, "{}", node.name);
        } else {
            let _ = write!(writer, "\x1b[1;36m{}\x1b[0m", node.name);
        }
    } else if raw {
        let _ = write!(writer, "{}", node.name);
    } else {
        let _ = write!(writer, "\x1b[0m{}\x1b[0m", node.name);
    }

    let metas = collect_metas(node, cfg);
    write_metadata(&metas, raw, writer);

    let _ = writeln!(writer);
}