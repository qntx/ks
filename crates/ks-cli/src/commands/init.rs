//! `ks init [--git | --clone <url>]` — create or clone a vault.

use ks::{ErrorCode, Vault};

use super::{Cli, CliResult, Out, err, fail, prompt};

/// Creates the vault (optionally a git repo) or clones an existing one.
pub(crate) fn run(cli: &Cli, out: &Out, git_flag: bool, clone: Option<&str>) -> CliResult<i32> {
    let path = cli.vault_path().map_err(|e| err(&e))?;

    if let Some(url) = clone {
        return clone_vault(cli, out, url, &path);
    }

    let passphrase = prompt::new_passphrase(out)?;
    let warnings = ks::check_new_passphrase(&passphrase).map_err(|e| err(&e))?;
    let _session = Vault::create(&path, &passphrase).map_err(|e| err(&e))?;
    for w in warnings {
        out.warn(w);
    }
    let mut value = serde_json::json!({ "created": true, "path": path.display().to_string() });
    if git_flag {
        let git = ks::git::Git::new(&path, out.interactive);
        git.init("ks: init").map_err(|e| err(&e))?;
        if let Some(map) = value.as_object_mut() {
            map.insert("git".to_owned(), serde_json::Value::from(true));
        }
    }
    Ok(out.ok_value(&format!("created vault at {}", path.display()), value))
}

/// `git clone` into `path` (must not exist or be empty), then unlock to
/// confirm the passphrase works; a failed unlock keeps the clone.
fn clone_vault(cli: &Cli, out: &Out, url: &str, path: &std::path::Path) -> CliResult<i32> {
    if path.exists() && std::fs::read_dir(path).is_ok_and(|mut d| d.next().is_some()) {
        return fail(
            ErrorCode::Exists,
            format!("{} is not empty", path.display()),
        );
    }
    let _git = ks::git::Git::clone(url, path, out.interactive).map_err(|e| err(&e))?;
    let v = Vault::open(path).map_err(|e| match e {
        ks::Error::NotFound(_) => (
            ErrorCode::NotFound,
            "the cloned repository contains no ks vault (.identity.age is missing); check the remote's default branch"
                .to_owned(),
        ),
        other => err(&other),
    })?;
    let source = prompt::passphrase_source(cli, out);
    let session = v.unlock(Some(&source)).map_err(|e| err(&e))?;
    for w in session.warnings() {
        out.warn(w.clone());
    }
    Ok(out.ok_value(
        &format!("cloned vault to {}", path.display()),
        serde_json::json!({ "cloned": true, "path": path.display().to_string() }),
    ))
}
