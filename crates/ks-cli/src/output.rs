//! Formatted terminal output helpers.
//!
//! All diagnostic output goes to **stderr**; only raw secret values go to **stdout**.

use owo_colors::OwoColorize as _;

pub fn print_success(msg: &str) {
    eprintln!("{} {}", "◆".green().bold(), msg.bold());
}

pub fn print_info(msg: &str) {
    eprintln!("{} {}", "●".cyan(), msg);
}

pub fn print_warn(msg: &str) {
    eprintln!("{} {}", "⚠".yellow(), msg);
}

pub fn print_error(msg: &str) {
    eprintln!("{} {}", "✗".red().bold(), msg);
}

/// Renders a list of secret paths as a Unicode tree, writing to stderr.
pub fn print_tree(paths: &[&str]) {
    if paths.is_empty() {
        eprintln!("{}", "(empty)".dimmed());
        return;
    }

    struct Node {
        children: std::collections::BTreeMap<String, Self>,
        is_leaf: bool,
    }

    impl Node {
        const fn new() -> Self {
            Self {
                children: std::collections::BTreeMap::new(),
                is_leaf: false,
            }
        }

        fn insert(&mut self, parts: &[&str]) {
            if parts.is_empty() {
                self.is_leaf = true;
                return;
            }
            self.children
                .entry(parts[0].to_owned())
                .or_insert_with(Self::new)
                .insert(&parts[1..]);
        }
    }

    let mut root = Node::new();
    for path in paths {
        let parts: Vec<&str> = path.split('/').collect();
        root.insert(&parts);
    }

    fn render(node: &Node, prefix: &str, is_last: bool, depth: usize) {
        let connector = if is_last { "└── " } else { "├── " };
        let extension = if is_last { "    " } else { "│   " };

        let names: Vec<&String> = node.children.keys().collect();
        for (i, name) in names.iter().enumerate() {
            let child = &node.children[*name];
            let last = i == names.len() - 1;
            let has_children = !child.children.is_empty();

            if depth == 0 {
                if has_children {
                    eprintln!("{}", format!("{name}/").bold().cyan());
                } else {
                    eprintln!("{name}");
                }
                render(child, "", last, depth + 1);
            } else {
                let line_connector = if last { "└── " } else { "├── " };
                if has_children {
                    eprintln!(
                        "{}{}",
                        format!("{prefix}{line_connector}").dimmed(),
                        format!("{name}/").bold().cyan()
                    );
                } else {
                    eprintln!("{}{}", format!("{prefix}{line_connector}").dimmed(), name);
                }
                let new_prefix = format!("{prefix}{extension}");
                render(child, &new_prefix, last, depth + 1);
            }
        }
        let _ = connector;
    }

    render(&root, "", true, 0);
}
