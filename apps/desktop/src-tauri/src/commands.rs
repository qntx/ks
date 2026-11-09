//! Tauri commands over `ks` — secrets only cross via `SecretText`.

use serde::Serialize;
use specta::Type;

use sha2::Digest as _;

use crate::error::AppError;
use crate::secret::SecretText;
use crate::state::AppState;

type R<T> = Result<T, AppError>;

/// Vault descriptor for lists.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VaultInfo {
    /// Vault root path.
    pub path: String,
    /// Whether it is a git repo.
    pub is_git: bool,
    /// Non-fatal warnings (e.g. a skipped initial commit).
    pub warnings: Vec<String>,
}

fn info(v: &ks::Vault) -> VaultInfo {
    info_with_warnings(v, Vec::new())
}

fn info_with_warnings(v: &ks::Vault, warnings: Vec<String>) -> VaultInfo {
    VaultInfo {
        path: v.root().display().to_string(),
        is_git: v.root().join(".git").exists(),
        warnings,
    }
}

/// Full view for the shell.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppView {
    /// Open vault, if any.
    pub vault: Option<VaultInfo>,
    /// Whether a session is unlocked.
    pub unlocked: bool,
    /// Current settings.
    pub settings: crate::settings::Settings,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn app_state(state: tauri::State<'_, std::sync::Arc<AppState>>) -> AppView {
    let i = state
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    AppView {
        vault: i.open.as_ref().map(|o| info(&o.vault)),
        unlocked: i.open.as_ref().is_some_and(|o| o.session.is_some()),
        settings: crate::settings::load(),
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) fn recent_vaults() -> Vec<String> {
    let mut out = vec![];
    if let Ok(p) = ks::Vault::default_path() {
        out.push(p.display().to_string());
    }
    out
}

/// Opens an existing vault directory.
#[tauri::command]
#[specta::specta]
pub(crate) fn open_vault(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    path: String,
) -> R<VaultInfo> {
    let v = ks::Vault::open(std::path::Path::new(&path))?;
    let mut i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    i.open = Some(crate::state::Open {
        vault: ks::Vault::open(std::path::Path::new(&path))?,
        session: None,
        watcher: None,
        last_activity: std::time::Instant::now(),
        unlocked_at: None,
    });
    Ok(info(&v))
}

/// Creates a new vault.
#[tauri::command]
#[specta::specta]
pub(crate) fn create_vault(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    path: String,
    passphrase: SecretText,
    git: bool,
) -> R<VaultInfo> {
    let p = std::path::Path::new(&path);
    let _s = ks::Vault::create(
        p,
        &age::secrecy::SecretString::from(passphrase.expose().to_owned()),
    )?;
    if git {
        ks::git::Git::new(p, false).init("ks init")?;
    }
    open_vault(state, path)
}

/// Clones a remote vault (blocking).
#[tauri::command]
#[specta::specta]
pub(crate) async fn clone_vault(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    url: String,
    path: String,
    passphrase: SecretText,
) -> R<VaultInfo> {
    let v = tauri::async_runtime::spawn_blocking(move || {
        let g = ks::git::Git::clone(&url, std::path::Path::new(&path), false)?;
        drop(g);
        ks::Vault::open(std::path::Path::new(&path)).and_then(|v| {
            v.unlock_without_agent(Some(&|| {
                Ok(age::secrecy::SecretString::from(
                    passphrase.expose().to_owned(),
                ))
            }))?;
            Ok(v)
        })
    })
    .await
    .map_err(|e| AppError {
        code: "internal".into(),
        message: e.to_string(),
    })??;
    let out = info(&v);
    let mut i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    i.open = Some(crate::state::Open {
        vault: v,
        session: None,
        watcher: None,
        last_activity: std::time::Instant::now(),
        unlocked_at: None,
    });
    Ok(out)
}

/// Structured passphrase strength for live feedback.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Strength {
    /// zxcvbn 0-4.
    pub score: u8,
    /// Whether policy passes.
    pub ok: bool,
    /// First problem, if any.
    pub problem: Option<String>,
    /// zxcvbn suggestions.
    pub suggestions: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn passphrase_strength(passphrase: SecretText) -> Strength {
    let s = ks::strength(passphrase.expose());
    Strength {
        score: s.score,
        ok: s.ok,
        problem: s.problem,
        suggestions: s.suggestions,
    }
}

/// Unlock result.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnlockResult {
    /// Non-fatal unlock warnings.
    pub warnings: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn unlock(
    app: tauri::AppHandle,
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    passphrase: Option<SecretText>,
) -> R<UnlockResult> {
    let mut i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    let o = i.open.as_mut().ok_or(ks::Error::Locked)?;
    let text = passphrase.as_ref().map(|p| p.expose().to_owned());
    let cb = move || {
        Ok(age::secrecy::SecretString::from(
            text.clone().unwrap_or_default(),
        ))
    };
    let s = o.vault.unlock_without_agent(Some(&cb))?;
    let warnings: Vec<String> = s.warnings().to_vec();
    let root = o.vault.root().to_path_buf();
    let identity_hash = std::fs::read(root.join(".identity.age"))
        .ok()
        .map(|b| sha2::Sha256::digest(&b).to_vec())
        .unwrap_or_default();
    o.session = Some(s);
    o.unlocked_at = Some(std::time::Instant::now());
    o.last_activity = std::time::Instant::now();
    drop(i);
    // watcher needs the app handle + state: re-register via commands
    let mut i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    if let Some(o) = i.open.as_mut() {
        o.watcher =
            crate::watch::Watcher::start(app, std::sync::Arc::clone(&*state), root, identity_hash);
    }
    #[cfg(target_os = "macos")]
    #[cfg(target_os = "macos")]
    crate::lock::platform::begin_activity();
    let st = crate::settings::load();
    crate::lock::start(
        std::sync::Arc::clone(&*state),
        std::time::Duration::from_secs(u64::from(st.auto_lock_minutes) * 60),
        std::time::Duration::from_secs(u64::from(st.max_session_hours) * 3600),
    );
    Ok(UnlockResult { warnings })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn lock(state: tauri::State<'_, std::sync::Arc<AppState>>) -> R<()> {
    state.lock(crate::lock::Reason::Manual);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn heartbeat(state: tauri::State<'_, std::sync::Arc<AppState>>) {
    state.touch();
}

#[tauri::command]
#[specta::specta]
pub(crate) fn list_entries(state: tauri::State<'_, std::sync::Arc<AppState>>) -> R<Vec<String>> {
    state
        .with_session(|s| {
            s.vault()
                .list(None)
                .map(|ns| ns.iter().map(|n| n.to_string()).collect())
        })
        .map_err(AppError::from)
}

/// One non-secret field descriptor.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FieldInfo {
    /// Field key.
    pub key: String,
    /// Value length in bytes.
    pub length: u32,
}

/// Decrypted detail without values.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntryDetail {
    /// Entry name.
    pub name: String,
    /// "text" | "binary".
    pub kind: String,
    /// The main value's field info when text.
    pub value: Option<FieldInfo>,
    /// Extra fields (names + lengths only).
    pub fields: Vec<FieldInfo>,
    /// Notes byte length.
    pub notes_length: u32,
    /// Whether a `totp` field exists.
    pub has_totp: bool,
    /// Binary payload length when binary.
    pub binary_length: Option<u32>,
}

/// Which part of an entry to reveal/copy.
#[derive(Debug, serde::Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FieldRef {
    /// The main value.
    Value,
    /// A named field.
    Field(String),
    /// The notes text.
    Notes,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn entry_detail(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
) -> R<EntryDetail> {
    let entry = state.with_session(|s| s.get(&ks::Name::parse(&name)?))?;
    let (value, fields, notes_len, totp, kind, bin_len) = match entry.as_text() {
        Some(t) => {
            let mut fields = Vec::new();
            let mut notes_len = 0;
            let mut totp = false;
            for (k, v) in t.fields() {
                if k == "notes" {
                    notes_len = v.len() as u32;
                    continue;
                }
                if k == "totp" {
                    totp = true;
                    continue;
                }
                fields.push(FieldInfo {
                    key: k.to_owned(),
                    length: v.len() as u32,
                });
            }
            (
                Some(FieldInfo {
                    key: "value".to_owned(),
                    length: t.value().len() as u32,
                }),
                fields,
                notes_len,
                totp,
                "text".to_owned(),
                None,
            )
        }
        None => (
            None,
            Vec::new(),
            0,
            false,
            "binary".to_owned(),
            Some(entry.payload().len() as u32),
        ),
    };
    Ok(EntryDetail {
        name,
        kind,
        value,
        fields,
        notes_length: notes_len,
        has_totp: totp,
        binary_length: bin_len,
    })
}

fn resolve(entry: &ks::Entry, field: &FieldRef) -> Result<String, ks::Error> {
    let t = entry
        .as_text()
        .ok_or_else(|| ks::Error::InvalidInput("binary entries cannot be revealed".into()))?;
    match field {
        FieldRef::Value => Ok(t.value().to_owned()),
        FieldRef::Field(k) => t
            .field(k)
            .map(str::to_owned)
            .ok_or_else(|| ks::Error::NotFound(k.clone())),
        FieldRef::Notes => t
            .field("notes")
            .map(str::to_owned)
            .ok_or_else(|| ks::Error::NotFound("notes".to_owned())),
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) fn reveal_field(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
    field: FieldRef,
) -> R<SecretText> {
    let e = state.with_session(|s| s.get(&ks::Name::parse(&name)?))?;
    Ok(SecretText::new(resolve(&e, &field)?))
}

/// Copy result: when the clipboard clears.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Copied {
    /// Seconds until cleared.
    pub clears_in_secs: u32,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn copy_field(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
    field: FieldRef,
) -> R<Copied> {
    let e = state.with_session(|s| s.get(&ks::Name::parse(&name)?))?;
    let v = resolve(&e, &field)?;
    let copied = ks_clipboard::copy(&v).map_err(|e| AppError {
        code: "unavailable".into(),
        message: e.to_string(),
    })?;
    let secs = u64::from(crate::settings::load().clipboard_seconds);
    std::thread::spawn(move || {
        #[expect(
            clippy::disallowed_methods,
            reason = "clipboard clear timer: blocking sleep in a dedicated thread is the simplest reliable timing"
        )]
        std::thread::sleep(std::time::Duration::from_secs(secs));
        drop(ks_clipboard::clear_if_unchanged(&copied));
    });
    Ok(Copied {
        clears_in_secs: u32::try_from(secs).unwrap_or(u32::MAX),
    })
}

/// Current TOTP code + validity.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TotpCode {
    /// The code.
    pub code: String,
    /// Seconds until rollover.
    pub valid_for: u32,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn totp_code(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
) -> R<TotpCode> {
    let e = state.with_session(|s| s.get(&ks::Name::parse(&name)?))?;
    let (code, valid_for) = ks::totp::code(&e, std::time::SystemTime::now())?;
    Ok(TotpCode {
        code,
        valid_for: u32::try_from(valid_for).unwrap_or(30),
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn edit_entry(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
) -> R<SecretText> {
    let e = state.with_session(|s| s.get(&ks::Name::parse(&name)?))?;
    let t = e.as_text().ok_or(ks::Error::InvalidInput(
        "binary entries cannot be edited".into(),
    ))?;
    Ok(SecretText::new(t.as_str().to_owned()))
}

fn autocommit(v: &ks::Vault, msg: &str) -> Vec<String> {
    if !v.root().join(".git").exists() {
        return Vec::new();
    }
    match Ok::<_, ks::Error>(ks::git::Git::new(v.root(), false)) {
        Ok(g) => g
            .commit_paths(&[std::path::PathBuf::from("*.age")], msg)
            .map(|_| Vec::<String>::new())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) fn save_entry(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
    text: SecretText,
    overwrite: bool,
) -> R<Vec<String>> {
    state.with_session(|s| {
        s.set(
            &ks::Name::parse(&name)?,
            &ks::Entry::text(zeroize::Zeroizing::new(text.expose().to_owned()))?,
            overwrite,
        )
    })?;
    let i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    Ok(i.open
        .as_ref()
        .map_or_else(Vec::new, |o| autocommit(&o.vault, &format!("set {name}"))))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn rename_entry(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    from: String,
    to: String,
    overwrite: bool,
) -> R<Vec<String>> {
    state
        .with_session(|s| s.rename(&ks::Name::parse(&from)?, &ks::Name::parse(&to)?, overwrite))?;
    let i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    Ok(i.open.as_ref().map_or_else(Vec::new, |o| {
        autocommit(&o.vault, &format!("mv {from} {to}"))
    }))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_entry(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
) -> R<Vec<String>> {
    state.with_session(|s| s.remove(&ks::Name::parse(&name)?))?;
    let i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    Ok(i.open
        .as_ref()
        .map_or_else(Vec::new, |o| autocommit(&o.vault, &format!("rm {name}"))))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn import_binary(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
    path: String,
    overwrite: bool,
) -> R<Vec<String>> {
    let src = std::path::Path::new(&path);
    if !src.is_absolute() {
        return Err(ks::Error::InvalidInput("path must be absolute".into()).into());
    }
    let meta = std::fs::symlink_metadata(src).map_err(|e| ks::Error::Io {
        path: src.to_path_buf(),
        source: e,
    })?;
    if meta.file_type().is_symlink() || !meta.file_type().is_file() {
        return Err(ks::Error::InvalidInput("path must be a regular file".into()).into());
    }
    if meta.len() > 64 * 1024 * 1024 {
        return Err(ks::Error::InvalidInput("file larger than 64 MiB".into()).into());
    }
    let bytes = std::fs::read(src).map_err(|e| ks::Error::Io {
        path: src.to_path_buf(),
        source: e,
    })?;
    state.with_session(|s| {
        s.set(
            &ks::Name::parse(&name)?,
            &ks::Entry::binary(zeroize::Zeroizing::new(bytes))?,
            overwrite,
        )
    })?;
    let i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    Ok(i.open.as_ref().map_or_else(Vec::new, |o| {
        autocommit(&o.vault, &format!("import {name}"))
    }))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn export_binary(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
    path: String,
) -> R<()> {
    let e = state.with_session(|s| s.get(&ks::Name::parse(&name)?))?;
    if e.as_text().is_some() {
        return Err(ks::Error::InvalidInput("text entries have no binary payload".into()).into());
    }
    let dst = std::path::Path::new(&path);
    if !dst.is_absolute() {
        return Err(ks::Error::InvalidInput("path must be absolute".into()).into());
    }
    std::fs::write(dst, e.payload()).map_err(|e| ks::Error::Io {
        path: dst.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn generate_password(length: u32, charset: String) -> R<SecretText> {
    let sets: &[u8] = match charset.as_str() {
        "alphanum" => b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
        "hex" => b"0123456789abcdef",
        "printable" => b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{};:,.<>/?",
        "slug" => b"abcdefghijklmnopqrstuvwxyz0123456789",
        _ => return Err(ks::Error::InvalidInput("unknown charset".into()).into()),
    };
    use rand::prelude::IndexedRandom as _;
    let mut rng = rand::rng();
    let mut out = String::with_capacity(length as usize);
    for _ in 0..length {
        if let Some(c) = sets.choose(&mut rng) {
            out.push(*c as char);
        }
    }
    Ok(SecretText::new(out))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_settings() -> crate::settings::Settings {
    crate::settings::load()
}

#[tauri::command]
#[specta::specta]
pub(crate) fn set_settings(s: crate::settings::Settings) -> R<()> {
    crate::settings::save(&s).map_err(|e| AppError {
        code: "io".into(),
        message: e.to_string(),
    })
}

/// Agent liveness.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInfo {
    /// Who serves it.
    pub host: String,
    /// Seconds until idle expiry.
    pub ttl_left: u32,
}

/// D0 status shape (kept for the shell).
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VaultStatusOut {
    /// Default vault path.
    pub path: String,
    /// Whether a vault exists there.
    pub exists: bool,
    /// Whether it is a git repo.
    pub is_git: bool,
    /// Agent liveness when running.
    pub agent: Option<AgentInfo>,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn vault_status() -> R<VaultStatusOut> {
    let path = ks::Vault::default_path()?;
    Ok(VaultStatusOut {
        path: path.display().to_string(),
        exists: path.is_dir(),
        is_git: path.join(".git").exists(),
        agent: None,
    })
}
