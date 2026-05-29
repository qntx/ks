//! Formatted, colourised terminal output.
//!
//! Convention:
//! - **stdout** carries user-facing data (secret values, listings, generated codes).
//!   Pipe-friendly.
//! - **stderr** carries diagnostics, prompts and progress feedback.
//!
//! All status helpers in this module write to **stderr**. Colour is applied
//! only when the destination stream is a colour-capable terminal (honouring
//! `NO_COLOR`), so redirected or piped output stays plain text.

use std::collections::BTreeMap;

use owo_colors::{OwoColorize as _, Stream, Style};

/// Prints a positive completion message (green diamond).
pub fn success(msg: &str) {
    eprintln!(
        "{} {}",
        "◆".if_supports_color(Stream::Stderr, |t| t.style(Style::new().green().bold())),
        msg.if_supports_color(Stream::Stderr, |t| t.bold()),
    );
}

/// Prints a neutral informational message (cyan dot).
pub fn info(msg: &str) {
    eprintln!(
        "{} {msg}",
        "●".if_supports_color(Stream::Stderr, |t| t.cyan()),
    );
}

/// Prints a soft warning (yellow caution).
pub fn warn(msg: &str) {
    eprintln!(
        "{} {msg}",
        "⚠".if_supports_color(Stream::Stderr, |t| t.yellow()),
    );
}

/// Prints an error (red cross), used by `main.rs` for the final failure line.
pub fn error(msg: &str) {
    eprintln!(
        "{} {}",
        "✗".if_supports_color(Stream::Stderr, |t| t.style(Style::new().red().bold())),
        msg.if_supports_color(Stream::Stderr, |t| t.bold()),
    );
}

/// Prints `paths` as a Unicode tree, one logical secret per line, to **stdout**.
pub fn tree(paths: &[&str]) {
    if paths.is_empty() {
        eprintln!(
            "{}",
            "(empty)".if_supports_color(Stream::Stderr, |t| t.dimmed()),
        );
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
            format!("{name}/")
                .if_supports_color(Stream::Stdout, |t| t.style(Style::new().cyan().bold()))
                .to_string()
        };

        if depth == 0 {
            println!("{label}");
        } else {
            println!(
                "{}{}{label}",
                prefix.if_supports_color(Stream::Stdout, |t| t.dimmed()),
                connector.if_supports_color(Stream::Stdout, |t| t.dimmed()),
            );
        }

        let new_prefix = if depth == 0 {
            String::new()
        } else {
            format!("{prefix}{extension}")
        };
        render(child, &new_prefix, depth + 1);
    }
}
