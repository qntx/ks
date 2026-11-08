//! Passphrase sources and interactive prompts.

use std::io::Read as _;

use age::secrecy::SecretString;
use ks::{Error, ErrorCode};

use crate::cli::Cli;
use crate::output::{CliResult, Out, err};

/// The unlock passphrase source handed to `Vault::unlock`.
///
/// Order: `--passphrase-file` -> `KS_PASSPHRASE_FILE` -> `KS_PASSPHRASE`
/// (captured at startup and removed from the environment) -> interactive
/// hidden prompt. Absent everything: `locked`.
pub fn passphrase_source<'a>(
    cli: &'a Cli,
    interactive: bool,
) -> impl Fn() -> ks::Result<SecretString> + 'a {
    move || {
        if let Some(file) = cli.passphrase_file.as_deref().or_env("KS_PASSPHRASE_FILE") {
            return read_passphrase_file(std::path::Path::new(&file));
        }
        if let Some(pass) = crate::harden::captured_passphrase() {
            return Ok(pass);
        }
        if interactive {
            return prompt_hidden("master passphrase");
        }
        Err(Error::Locked)
    }
}

trait OrEnv {
    fn or_env(self, var: &str) -> Option<String>;
}
impl OrEnv for Option<&str> {
    fn or_env(self, var: &str) -> Option<String> {
        self.map(str::to_owned)
            .or_else(|| std::env::var(var).ok().filter(|v| !v.is_empty()))
    }
}

/// Reads a passphrase file, stripping one trailing `\r\n` or `\n`. Warns on
/// unix when the file is group/other readable (warning goes to stderr in
/// human mode via the caller — here we just read).
fn read_passphrase_file(path: &std::path::Path) -> ks::Result<SecretString> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if let Ok(meta) = path.metadata() {
            if meta.permissions().mode() & 0o077 != 0 {
                eprintln!("warning: passphrase file {path:?} is readable by group/other");
            }
        }
    }
    let mut bytes = std::fs::read(path).map_err(|e| Error::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    if bytes.ends_with(b"\r\n") {
        bytes.truncate(bytes.len() - 2);
    } else if bytes.ends_with(b"\n") {
        bytes.truncate(bytes.len() - 1);
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::InvalidInput("passphrase file is not UTF-8".into()))?;
    Ok(SecretString::from(text))
}

/// Hidden single-line prompt (cliclack).
fn prompt_hidden(label: &str) -> ks::Result<SecretString> {
    let input = cliclack::password(label)
        .interact()
        .map_err(|_| Error::Locked)?;
    Ok(SecretString::from(input))
}

/// Acquires a NEW passphrase: interactive -> typed twice; non-interactive ->
/// `KS_NEW_PASSPHRASE_FILE`, else `usage`.
pub fn new_passphrase(out: &Out) -> CliResult<SecretString> {
    if out.interactive {
        let first = cliclack::password("new passphrase")
            .interact()
            .map_err(|_| (ErrorCode::Cancelled, "cancelled".to_owned()))?;
        let second = cliclack::password("confirm new passphrase")
            .interact()
            .map_err(|_| (ErrorCode::Cancelled, "cancelled".to_owned()))?;
        if first != second {
            return Err((ErrorCode::Usage, "passphrases do not match".to_owned()));
        }
        return Ok(SecretString::from(first));
    }
    match std::env::var("KS_NEW_PASSPHRASE_FILE") {
        Ok(file) if !file.is_empty() => {
            read_passphrase_file(std::path::Path::new(&file)).map_err(|e| err(&e))
        }
        _ => Err((
            ErrorCode::Usage,
            "new passphrase required: set KS_NEW_PASSPHRASE_FILE non-interactively".to_owned(),
        )),
    }
}

/// Reads the first line of stdin for `set` (trailing `\n`/`\r\n` removed).
pub fn stdin_first_line() -> CliResult<String> {
    let mut line = String::new();
    let n = std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| (ErrorCode::Io, format!("reading stdin: {e}")))?;
    if n == 0 {
        return Err((ErrorCode::Usage, "empty input on stdin".to_owned()));
    }
    if line.ends_with("\r\n") {
        line.truncate(line.len() - 2);
    } else if line.ends_with('\n') {
        line.truncate(line.len() - 1);
    }
    Ok(line)
}

/// Reads all of stdin as bytes.
pub fn stdin_all() -> CliResult<Vec<u8>> {
    let mut buf = Vec::new();
    std::io::stdin()
        .read_to_end(&mut buf)
        .map_err(|e| (ErrorCode::Io, format!("reading stdin: {e}")))?;
    Ok(buf)
}

/// Interactive yes/no confirmation; non-interactive returns the `missing`
/// error supplied by the caller.
pub fn confirm(out: &Out, question: &str) -> CliResult<()> {
    if !out.interactive {
        return Err((
            ErrorCode::Usage,
            format!("{question} (requires confirmation or a flag)"),
        ));
    }
    let yes = cliclack::confirm(question)
        .initial_value(false)
        .interact()
        .map_err(|_| (ErrorCode::Cancelled, "cancelled".to_owned()))?;
    if !yes {
        return Err((ErrorCode::Cancelled, "cancelled".to_owned()));
    }
    Ok(())
}
