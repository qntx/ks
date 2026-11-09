//! Tauri commands over `ks` — secrets only cross via `SecretText`.

use serde::Serialize;
use specta::Type;

use sha2::Digest as _;

use age::secrecy::SecretString;

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
    /// Why the vault last locked, so a reloaded webview can show it.
    pub lock_reason: Option<crate::lock::Reason>,
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
        lock_reason: i.last_lock_reason,
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

/// Installs `vault` as the open (still locked) vault, dropping any
/// previous session, watcher and timers.
pub(crate) fn install_open(state: &std::sync::Arc<AppState>, vault: ks::Vault) -> R<VaultInfo> {
    let out = info(&vault);
    let mut i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    i.open = Some(crate::state::Open {
        agent: None,
        vault,
        session: None,
        watcher: None,
        watchdog: None,
        last_activity: std::time::Instant::now(),
        unlocked_at: None,
    });
    Ok(out)
}

/// Installs an unlocked `Session` on the open vault: activity stamps,
/// identity watcher, App Nap suppression and the auto-lock watchdog.
/// Returns the session's non-fatal warnings.
fn begin_session<RT: tauri::Runtime>(
    app: &tauri::AppHandle<RT>,
    state: &std::sync::Arc<AppState>,
    session: ks::Session,
) -> R<Vec<String>> {
    let mut warnings = session.warnings().to_vec();
    let agent_identity = session.agent_identity();

    // Under the lock: install the session, clocks, watcher, watchdog.
    let (root, installed_at, idle, max) = {
        let mut i = state
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        let o = i.open.as_mut().ok_or(ks::Error::Locked)?;
        let root = o.vault.root().to_path_buf();
        let identity_hash = std::fs::read(root.join(".identity.age"))
            .ok()
            .map(|b| sha2::Sha256::digest(&b).to_vec())
            .unwrap_or_default();
        let at = std::time::Instant::now();
        o.session = Some(session);
        o.unlocked_at = Some(at);
        o.last_activity = at;
        o.watcher = crate::watch::Watcher::start(
            app.clone(),
            std::sync::Arc::clone(state),
            root.clone(),
            identity_hash,
        );
        #[cfg(target_os = "macos")]
        drop(app.run_on_main_thread(crate::lock::platform::begin_activity));
        let st = crate::settings::load();
        let idle = duration_or_never(st.auto_lock_minutes, 60);
        let max = duration_or_never(st.max_session_hours, 3600);
        o.start_watchdog(std::sync::Arc::clone(state), idle, max);
        i.last_lock_reason = None;
        (root, at, idle, max)
    };

    // Host the agent endpoint so the CLI/agent work without a passphrase
    // (ADR-16) — outside the state lock: `ks::agent::lock` can block ~2 s
    // while a superseded agent exits, and `Server::start` binds a socket.
    // A running `ks unlock` agent is asked to exit first; the app then
    // takes over. Failure does not block the session.
    let started = ks::Vault::open(&root)
        .map_err(|e| e.to_string())
        .and_then(|v| {
            drop(ks::agent::lock(&v));
            agent_identity
                .ok_or_else(|| "vault has no exportable identity".to_owned())
                .and_then(|identity| {
                    ks::agent::Server::start(
                        &v,
                        identity,
                        ks::agent::ServerConfig {
                            ttl: idle,
                            max,
                            host: ks::agent::Host::Desktop,
                        },
                    )
                    .map_err(|e| e.to_string())
                })
        });
    match started {
        Ok(server) => {
            let mut i = state
                .inner
                .lock()
                .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
            let same_session = i
                .open
                .as_ref()
                .is_some_and(|o| o.session.is_some() && o.unlocked_at == Some(installed_at));
            if same_session {
                if let Some(o) = i.open.as_mut() {
                    o.agent = Some(server.handle());
                }
                let st2 = std::sync::Arc::clone(state);
                drop(i);
                std::thread::spawn(move || wait_agent(server, st2));
            } else {
                // Locked or replaced while the endpoint was being set up.
                drop(i);
                let _ = server.stop();
            }
        }
        Err(e) => warnings.push(format!("terminal session unavailable: {e}")),
    }
    crate::sync::after_unlock(state);
    Ok(warnings)
}

/// Agent waiter thread: `Server::wait` blocks until the endpoint dies,
/// then maps the stop reason to an app lock (`Stopped` needs none).
fn wait_agent(server: ks::agent::Server, state: std::sync::Arc<AppState>) {
    let reason = match server.wait() {
        ks::agent::StopReason::Idle => Some(crate::lock::Reason::Idle),
        ks::agent::StopReason::MaxLifetime => Some(crate::lock::Reason::Max),
        ks::agent::StopReason::LockRequested => Some(crate::lock::Reason::Terminal),
        ks::agent::StopReason::Stopped => None,
    };
    if let Some(reason) = reason {
        let _ = state.lock(reason);
    }
}

/// Settings encode "never" as `0`; map it to an unbounded duration.
fn duration_or_never(n: u32, unit_secs: u64) -> std::time::Duration {
    if n == 0 {
        std::time::Duration::MAX
    } else {
        std::time::Duration::from_secs(u64::from(n) * unit_secs)
    }
}

/// Opens an existing vault directory.
#[tauri::command]
#[specta::specta]
pub(crate) fn open_vault(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    path: String,
) -> R<VaultInfo> {
    let v = ks::Vault::open(std::path::Path::new(&path))?;
    crate::settings::remember_vault(v.root());
    install_open(state.inner(), v)
}

/// Creates a new vault and returns it already unlocked.
#[tauri::command]
#[specta::specta]
pub(crate) fn create_vault<RT: tauri::Runtime>(
    app: tauri::AppHandle<RT>,
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    path: String,
    passphrase: SecretText,
    git: bool,
) -> R<VaultInfo> {
    let p = std::path::Path::new(&path);
    let session = ks::Vault::create(p, &SecretString::from(passphrase.expose().to_owned()))?;
    if git {
        ks::git::Git::new(p, false).init("ks init")?;
    }
    let v = ks::Vault::open(p)?;
    crate::settings::remember_vault(v.root());
    let mut out = install_open(state.inner(), v)?;
    out.warnings = begin_session(&app, state.inner(), session)?;
    Ok(out)
}

/// Clones a remote vault (blocking) and returns it already unlocked.
#[tauri::command]
#[specta::specta]
pub(crate) async fn clone_vault<RT: tauri::Runtime>(
    app: tauri::AppHandle<RT>,
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    url: String,
    path: String,
    passphrase: SecretText,
) -> R<VaultInfo> {
    let session = tauri::async_runtime::spawn_blocking(move || {
        let g = ks::git::Git::clone(&url, std::path::Path::new(&path), false)?;
        drop(g);
        ks::Vault::open(std::path::Path::new(&path)).and_then(|v| {
            v.unlock_without_agent(Some(&|| {
                Ok(SecretString::from(passphrase.expose().to_owned()))
            }))
        })
    })
    .await
    .map_err(|e| AppError {
        code: "internal".into(),
        message: e.to_string(),
    })??;
    let v = ks::Vault::open(session.vault().root())?;
    crate::settings::remember_vault(v.root());
    let mut out = install_open(state.inner(), v)?;
    out.warnings = begin_session(&app, state.inner(), session)?;
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
pub(crate) fn unlock<RT: tauri::Runtime>(
    app: tauri::AppHandle<RT>,
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    passphrase: SecretText,
    retire_stale: bool,
) -> R<UnlockResult> {
    let session = {
        let mut i = state
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        let o = i.open.as_mut().ok_or(ks::Error::Locked)?;
        let text = passphrase.expose().to_owned();
        let cb = move || Ok(SecretString::from(text.clone()));
        if retire_stale {
            // Full chain: the passphrase also proves the identity is
            // genuine, so stale device sources are removed (DEVICE-STALE).
            o.vault.unlock_without_agent(Some(&cb))?
        } else {
            o.vault.unlock_passphrase(&cb)?
        }
    };
    let warnings = begin_session(&app, state.inner(), session)?;
    Ok(UnlockResult { warnings })
}

/// Outcome of the device-unlock attempt for the unlock screen.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceUnlockResult {
    /// A device session was established.
    pub unlocked: bool,
    /// Session plus device warnings; surfaced as toasts, not errors.
    pub warnings: Vec<String>,
    /// A device source held an outdated key — the passphrase will remove
    /// it on the next successful unlock.
    pub stale: bool,
}

/// Availability of one device source for the settings group.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceState {
    /// The platform offers the source at all.
    pub supported: bool,
    /// The vault currently has the source enabled.
    pub enabled: bool,
    /// Why the source is unusable, when it is.
    pub reason: Option<String>,
}

/// All device sources the settings group and unlock screen render.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Devices {
    /// Touch ID via `age-plugin-se`.
    pub touch_id: DeviceState,
    /// OS keychain entry.
    pub keychain: DeviceState,
}

/// The device source to toggle, serialized as `"touch_id" | "keychain"`.
#[derive(Debug, Serialize, serde::Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeviceKind {
    /// Touch ID via `age-plugin-se`.
    TouchId,
    /// OS keychain entry.
    Keychain,
}

/// Tries the device sources only (ADR-16 companion: Touch ID prompt or a
/// silent keychain read). Never asks for a passphrase.
#[tauri::command]
#[specta::specta]
pub(crate) fn unlock_device<RT: tauri::Runtime>(
    app: tauri::AppHandle<RT>,
    state: tauri::State<'_, std::sync::Arc<AppState>>,
) -> R<DeviceUnlockResult> {
    let out = {
        let i = state
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        let o = i.open.as_ref().ok_or(ks::Error::Locked)?;
        o.vault.unlock_devices()?
    };
    match out {
        ks::DeviceUnlock::Unlocked(session) => {
            let warnings = begin_session(&app, state.inner(), session)?;
            Ok(DeviceUnlockResult {
                unlocked: true,
                warnings,
                stale: false,
            })
        }
        ks::DeviceUnlock::Unavailable { warnings, stale } => Ok(DeviceUnlockResult {
            unlocked: false,
            warnings,
            stale,
        }),
    }
}

/// Reads the device-source state without ever prompting (attribute-only
/// keychain query, PATH scan for `age-plugin-se`).
#[tauri::command]
#[specta::specta]
pub(crate) fn devices(state: tauri::State<'_, std::sync::Arc<AppState>>) -> R<Devices> {
    let i = state
        .inner
        .lock()
        .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
    let o = i.open.as_ref().ok_or(ks::Error::Locked)?;
    let touch_id = touch_id_state(&o.vault);
    let keychain = keychain_state(&o.vault);
    Ok(Devices { touch_id, keychain })
}

/// Enables or disables a device source. Enabling needs the unlocked local
/// key (`with_session` — agent sessions fail with `usage`); disabling
/// needs only an open vault.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_device(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    kind: DeviceKind,
    enabled: bool,
) -> R<Vec<String>> {
    let warnings = match (kind, enabled) {
        (DeviceKind::TouchId, true) => {
            state.with_session(|s| ks::touchid::enable(s).map(|()| Vec::new()))?
        }
        (DeviceKind::Keychain, true) => state.with_session(ks::keychain::enable)?,
        (DeviceKind::TouchId, false) => {
            let i = state
                .inner
                .lock()
                .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
            let o = i.open.as_ref().ok_or(ks::Error::Locked)?;
            ks::touchid::disable(&o.vault)?;
            Vec::new()
        }
        (DeviceKind::Keychain, false) => {
            let i = state
                .inner
                .lock()
                .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
            let o = i.open.as_ref().ok_or(ks::Error::Locked)?;
            ks::keychain::disable(&o.vault)?;
            Vec::new()
        }
    };
    Ok(warnings)
}

/// Touch ID availability: macOS and `age-plugin-se` on PATH. Failures of
/// `is_enabled` downgrade to `enabled: false` plus a reason — never a
/// prompt, never a hard error for a status query.
#[cfg(target_os = "macos")]
fn touch_id_state(vault: &ks::Vault) -> DeviceState {
    let supported = which::which("age-plugin-se").is_ok();
    match ks::touchid::is_enabled(vault) {
        Ok(enabled) if supported => DeviceState {
            supported: true,
            enabled,
            reason: None,
        },
        Ok(enabled) => DeviceState {
            supported: false,
            enabled,
            reason: Some(
                "age-plugin-se was not found; install it with `brew install age-plugin-se`"
                    .to_owned(),
            ),
        },
        Err(e) => DeviceState {
            supported,
            enabled: false,
            reason: Some(e.to_string()),
        },
    }
}

#[cfg(not(target_os = "macos"))]
fn touch_id_state(_vault: &ks::Vault) -> DeviceState {
    DeviceState {
        supported: false,
        enabled: false,
        reason: Some("Touch ID is only available on macOS".to_owned()),
    }
}

#[cfg(any(target_os = "macos", windows))]
fn keychain_state(vault: &ks::Vault) -> DeviceState {
    match ks::keychain::is_enabled(vault) {
        Ok(enabled) => DeviceState {
            supported: true,
            enabled,
            reason: None,
        },
        Err(e) => DeviceState {
            supported: true,
            enabled: false,
            reason: Some(e.to_string()),
        },
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn keychain_state(_vault: &ks::Vault) -> DeviceState {
    DeviceState {
        supported: false,
        enabled: false,
        reason: Some("the OS keychain is not available on this platform".to_owned()),
    }
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
    /// The current one-time code; never the seed.
    Totp,
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
        FieldRef::Totp => Ok(ks::totp::code(entry, std::time::SystemTime::now())?.0),
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

/// Autocommit of the caller's message. Writes take `state.git` before
/// the file op, so a running sync can never stage a half-written
/// change — attribution is deterministic.
fn autocommit(root: &std::path::Path, msg: &str) -> Vec<String> {
    if !root.join(".git").exists() {
        return Vec::new();
    }
    ks::git::Git::new(root, false)
        .commit_paths(&[std::path::PathBuf::from("*.age")], msg)
        .map(|_| Vec::<String>::new())
        .unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub(crate) fn save_entry(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
    text: SecretText,
    overwrite: bool,
) -> R<Vec<String>> {
    let _git = state
        .git
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.with_session(|s| {
        s.set(
            &ks::Name::parse(&name)?,
            &ks::Entry::text(zeroize::Zeroizing::new(text.expose().to_owned()))?,
            overwrite,
        )
    })?;
    let root = {
        let i = state
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        i.open.as_ref().map(|o| o.vault.root().to_path_buf())
    };
    crate::sync::after_write(state.inner());
    Ok(root.map_or_else(Vec::new, |r| autocommit(&r, &format!("set {name}"))))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn rename_entry(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    from: String,
    to: String,
    overwrite: bool,
) -> R<Vec<String>> {
    let _git = state
        .git
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state
        .with_session(|s| s.rename(&ks::Name::parse(&from)?, &ks::Name::parse(&to)?, overwrite))?;
    let root = {
        let i = state
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        i.open.as_ref().map(|o| o.vault.root().to_path_buf())
    };
    crate::sync::after_write(state.inner());
    Ok(root.map_or_else(Vec::new, |r| autocommit(&r, &format!("mv {from} {to}"))))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_entry(
    state: tauri::State<'_, std::sync::Arc<AppState>>,
    name: String,
) -> R<Vec<String>> {
    let _git = state
        .git
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.with_session(|s| s.remove(&ks::Name::parse(&name)?))?;
    let root = {
        let i = state
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        i.open.as_ref().map(|o| o.vault.root().to_path_buf())
    };
    crate::sync::after_write(state.inner());
    Ok(root.map_or_else(Vec::new, |r| autocommit(&r, &format!("rm {name}"))))
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
    let _git = state
        .git
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.with_session(|s| {
        s.set(
            &ks::Name::parse(&name)?,
            &ks::Entry::binary(zeroize::Zeroizing::new(bytes))?,
            overwrite,
        )
    })?;
    let root = {
        let i = state
            .inner
            .lock()
            .map_err(|_| ks::Error::Internal("state lock poisoned"))?;
        i.open.as_ref().map(|o| o.vault.root().to_path_buf())
    };
    crate::sync::after_write(state.inner());
    Ok(root.map_or_else(Vec::new, |r| autocommit(&r, &format!("import {name}"))))
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
