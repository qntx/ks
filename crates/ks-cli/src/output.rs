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
        use std::io::Write as _;
        writeln!(std::io::stderr(), "error: {message}").ok();
    }
    exit_code(code)
}

/// Output sink bound to the display mode.
#[derive(Debug)]
pub struct Out {
    /// True when `--json` was passed.
    pub json: bool,
    /// Warnings collected on the way to a result.
    pub warnings: std::cell::RefCell<Vec<String>>,
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
            warnings: std::cell::RefCell::new(Vec::new()),
            interactive,
        }
    }

    /// Adds a warning; human mode also echoes it on stderr immediately.
    pub fn warn(&self, msg: impl Into<String>) {
        use std::io::Write as _;
        let msg = msg.into();
        if !self.json {
            writeln!(std::io::stderr(), "warning: {msg}").ok();
        }
        self.warnings.borrow_mut().push(msg);
    }

    /// Emits a successful result: `human` on stdout (or JSON object).
    pub fn ok_value(&self, human: &str, value: serde_json::Value) -> i32 {
        use std::io::Write as _;
        if self.json {
            let mut obj = value;
            let warnings = self.warnings.borrow();
            if !warnings.is_empty()
                && let Some(map) = obj.as_object_mut()
            {
                map.insert("warnings".to_owned(), serde_json::json!(*warnings));
            }
            writeln!(std::io::stdout(), "{obj}").ok();
        } else if !human.is_empty() {
            writeln!(std::io::stdout(), "{human}").ok();
        }
        0
    }

    /// Emits a successful result with no payload.
    pub fn ok_silent(&self, value: serde_json::Value) -> i32 {
        self.ok_value("", value)
    }

    /// Emits an error and returns its exit code.
    pub fn error(&self, code: ErrorCode, message: &str) -> i32 {
        exit_error(self.json, code, message)
    }
}
