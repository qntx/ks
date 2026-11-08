//! Output: exactly one JSON object on stdout in `--json` mode; human output
//! sends values to stdout and everything else to stderr.

use ks::ErrorCode;

/// CLI-level result: exit code or (error code, message).
pub type CliResult<T> = Result<T, (ErrorCode, String)>;

/// Maps an [`ErrorCode`] to its process exit code.
pub fn exit_code(code: ErrorCode) -> i32 {
    match code {
        ErrorCode::Usage => 64,
        ErrorCode::Corrupt | ErrorCode::WeakPassphrase => 65,
        ErrorCode::NotFound => 66,
        ErrorCode::Unavailable => 69,
        ErrorCode::Internal => 70,
        ErrorCode::Exists => 73,
        ErrorCode::Io => 74,
        ErrorCode::Locked | ErrorCode::WrongPassphrase => 77,
        ErrorCode::Cancelled => 1,
        _ => 70,
    }
}

/// Converts a library error into the CLI error shape.
pub fn err(e: &ks::Error) -> (ErrorCode, String) {
    (e.code(), e.to_string())
}

/// Prints an error and returns its exit code (used before `Out` exists).
pub fn exit_error(json: bool, code: ErrorCode, message: &str) -> i32 {
    if json {
        let obj = serde_json::json!({ "error": { "code": code.as_str(), "message": message } });
        use std::io::Write as _;
        writeln!(std::io::stdout(), "{obj}").ok();
    } else {
        eprintln!("error: {message}");
    }
    exit_code(code)
}

/// Output sink bound to the display mode.
#[derive(Debug)]
pub struct Out {
    /// True when `--json` was passed.
    pub json: bool,
    /// Warnings collected on the way to a result.
    pub warnings: Vec<String>,
    /// Whether stdin and stderr are terminals and `--json` is absent.
    pub interactive: bool,
}

impl Out {
    /// Builds the sink; `interactive` mirrors the cli.mdx rule.
    pub fn new(json: bool) -> Self {
        use std::io::IsTerminal as _;
        let interactive =
            !json && std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
        Self {
            json,
            warnings: Vec::new(),
            interactive,
        }
    }

    /// Adds a warning; human mode also echoes it on stderr immediately.
    pub fn warn(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if !self.json {
            eprintln!("warning: {msg}");
        }
        self.warnings.push(msg);
    }

    /// Emits a successful result: `human` on stdout (or JSON object).
    pub fn ok_value(&mut self, human: &str, value: serde_json::Value) -> i32 {
        if self.json {
            let mut obj = value;
            if !self.warnings.is_empty() {
                obj["warnings"] = serde_json::json!(self.warnings);
            }
            use std::io::Write as _;
            writeln!(std::io::stdout(), "{obj}").ok();
        } else if !human.is_empty() {
            println!("{human}");
        }
        0
    }

    /// Emits a successful result with no payload.
    pub fn ok_silent(&mut self, value: serde_json::Value) -> i32 {
        self.ok_value("", value)
    }

    /// Emits an error and returns its exit code.
    pub fn error(&mut self, code: ErrorCode, message: &str) -> i32 {
        exit_error(self.json, code, message)
    }
}
