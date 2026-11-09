//! Tray icon and its localized menu. The menu text lives only in the
//! frontend dictionaries: on load (and on every language change) the UI
//! sends `TrayLabels` through `set_tray_labels`; before that the menu
//! stays English. State is Tauri-managed (`Tray`), so a missing tray
//! never blocks or panics — `refresh`/`set_labels` degrade to no-ops.

use std::sync::{Mutex, PoisonError};

use tauri::Manager;
use tauri::menu::{Menu, MenuItem};

/// The visible tray labels. `unlocked` is a prefix such as "Unlocked";
/// Rust appends " · `<vault dir name>`".
#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrayLabels {
    pub(crate) open: String,
    pub(crate) lock: String,
    pub(crate) quit: String,
    pub(crate) locked: String,
    pub(crate) unlocked: String,
}

impl Default for TrayLabels {
    fn default() -> Self {
        Self {
            open: "Open ks".to_owned(),
            lock: "Lock now".to_owned(),
            quit: "Quit".to_owned(),
            locked: "Locked".to_owned(),
            unlocked: "Unlocked".to_owned(),
        }
    }
}

/// Menu handles kept for `refresh`; `None` if the tray failed to build.
struct Items {
    status: MenuItem<tauri::Wry>,
    lock: MenuItem<tauri::Wry>,
}

/// Managed tray state — `labels` starts as the English defaults.
pub(crate) struct Tray {
    items: Mutex<Option<Items>>,
    labels: Mutex<TrayLabels>,
}

impl Default for Tray {
    fn default() -> Self {
        Self {
            items: Mutex::new(None),
            labels: Mutex::new(TrayLabels::default()),
        }
    }
}

/// Builds the status line: the locked label, or
/// "`<unlocked>` · `<vault dir name>`".
pub(crate) fn status_text(unlocked: bool, vault_name: &str, labels: &TrayLabels) -> String {
    if unlocked {
        format!("{} · {}", labels.unlocked, vault_name)
    } else {
        labels.locked.clone()
    }
}

/// Stores localized labels and refreshes the menu.
pub(crate) fn set_labels<RT: tauri::Runtime>(app: &tauri::AppHandle<RT>, next: TrayLabels) {
    if let Some(tray) = app.try_state::<Tray>() {
        *tray.labels.lock().unwrap_or_else(PoisonError::into_inner) = next;
    }
    refresh(app);
}

/// Re-applies the status line and Lock enablement from `AppState`.
/// Called after unlock, after every lock and after a vault opens.
/// Lock order: tray mutexes are only ever taken one at a time and never
/// across `inner` — values are cloned out first.
pub(crate) fn refresh<RT: tauri::Runtime>(app: &tauri::AppHandle<RT>) {
    let Some(tray) = app.try_state::<Tray>() else {
        return;
    };
    let labels = tray
        .labels
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let items = tray
        .items
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|i| (i.status.clone(), i.lock.clone()));
    let Some((status, lock_i)) = items else {
        return;
    };
    let (unlocked, name) = app
        .try_state::<std::sync::Arc<crate::state::AppState>>()
        .map_or_else(
            || (false, String::new()),
            |s| {
                let i = s.inner.lock().unwrap_or_else(PoisonError::into_inner);
                i.open.as_ref().map_or((false, String::new()), |o| {
                    (
                        o.session.is_some(),
                        o.vault
                            .root()
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    )
                })
            },
        );
    // Menu setters block until the main thread runs them; callers may
    // hold `state.git` while the main thread waits on it, so post the
    // update instead of waiting.
    let text = status_text(unlocked, &name, &labels);
    drop(app.run_on_main_thread(move || {
        drop(status.set_text(text));
        drop(lock_i.set_enabled(unlocked));
    }));
}

/// Builds the tray icon: status (disabled), separator, "Lock now",
/// "Open ks", separator, "Quit".
pub(crate) fn build(app: &tauri::App) -> tauri::Result<()> {
    let labels = app
        .try_state::<Tray>()
        .map(|t| {
            t.labels
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        })
        .unwrap_or_default();
    let status = MenuItem::with_id(
        app,
        "status",
        status_text(false, "", &labels),
        false,
        None::<&str>,
    )?;
    let lock_i = MenuItem::with_id(app, "lock", &labels.lock, false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", &labels.open, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", &labels.quit, true, None::<&str>)?;
    let sep1 = tauri::menu::PredefinedMenuItem::separator(app)?;
    let sep2 = tauri::menu::PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&status, &sep1, &lock_i, &open, &sep2, &quit])?;
    let Some(icon) = app.default_window_icon().cloned() else {
        return Ok(());
    };
    tauri::tray::TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .on_menu_event(|handle, event| match event.id.as_ref() {
            "open" => crate::window::show_main(handle),
            "lock" => {
                if let Some(state) = handle.try_state::<std::sync::Arc<crate::state::AppState>>() {
                    state.lock(crate::lock::Reason::Manual);
                }
            }
            "quit" => {
                handle.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    if let Some(tray) = app.try_state::<Tray>() {
        *tray.items.lock().unwrap_or_else(PoisonError::into_inner) = Some(Items {
            status,
            lock: lock_i,
        });
    }
    refresh(app.handle());
    Ok(())
}
