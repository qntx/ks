//! Formatted, colourised terminal output.
//!
//! Convention:
//! - **stdout** carries user-facing data (secret values, listings, generated codes).
//!   Pipe-friendly.
//! - **stderr** carries diagnostics, prompts and progress feedback.
//!
//! All status helpers in this module write to **stderr**.

use std::collections::BTreeMap;

use owo_colors::OwoColorize as _;

/// Prints a positive completion message (green diamond).
pub fn success(msg: &str) {
    eprintln!("{} {}", "◆".green().bold(), msg.bold());
}

/// Prints a neutral informational message (cyan dot).
pub fn info(msg: &str) {
    eprintln!("{} {}", "●".cyan(), msg);
}

/// Prints a soft warning (yellow caution).
pub fn warn(msg: &str) {
    eprintln!("{} {}", "⚠".yellow(), msg);
}

/// Prints an error (red cross), used by `main.rs` for the final failure line.
pub fn error(msg: &str) {
    eprintln!("{} {}", "✗".red().bold(), msg);
}

/// Prints `paths` as a Unicode tree, one logical secret per line, to **stdout**.
pub fn tree(paths: &[&str]) {
    if paths.is_empty() {
        eprintln!("{}", "(empty)".dimmed());
        return;
    }

    let mut root = Node::default();
    for path in paths {
        root.insert(&path.split('/').collect::<Vec<_>>());
    }
    render(&root, "", 0);
}

#[derive(Default)]
struct Node {
    children: BTreeMap<String, Self>,
    is_leaf: bool,
}

impl Node {
    fn insert(&mut self, parts: &[&str]) {
        if let Some((head, tail)) = parts.split_first() {
            self.children
                .entry((*head).to_owned())
                .or_default()
                .insert(tail);
        } else {
            self.is_leaf = true;
        }
    }
}

fn render(node: &Node, prefix: &str, depth: usize) {
    let len = node.children.len();
    for (i, (name, child)) in node.children.iter().enumerate() {
        let is_last = i + 1 == len;
        let (connector, extension) = if is_last {
            ("└── ", "    ")
        } else {
            ("├── ", "│   ")
        };

        let label = if child.children.is_empty() {
            name.clone()
        } else {
            format!("{name}/").cyan().bold().to_string()
        };

        if depth == 0 {
            println!("{label}");
        } else {
            println!("{}{}{}", prefix.dimmed(), connector.dimmed(), label);
        }

        let new_prefix = if depth == 0 {
            String::new()
        } else {
            format!("{prefix}{extension}")
        };
        render(child, &new_prefix, depth + 1);
    }
}
