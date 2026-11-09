//! The global quick-access shortcut (tauri-plugin-global-shortcut).
//! Only the four presets below are accepted; `None` disables it. The
//! handler toggles the quick panel: shown → hide, hidden → `show_quick`.

use tauri_plugin_global_shortcut::{GlobalShortcutExt as _, Shortcut};

/// The registered presets (also the frontend `ChoiceMenu` order).
pub(crate) const PRESETS: [&str; 4] = [
    "CommandOrControl+Shift+Space",
    "Alt+Space",
    "Control+Alt+Space",
    "CommandOrControl+Shift+K",
];

pub(crate) const DEFAULT: &str = "CommandOrControl+Shift+Space";

/// `invalid_input` for unknown chords, `shortcut_unavailable` when the
/// OS refuses the grab (another app holds it).
pub(crate) fn validate(shortcut: &str) -> Result<(), crate::error::AppError> {
    if PRESETS.contains(&shortcut) {
        Ok(())
    } else {
        Err(crate::error::AppError {
            code: "invalid_input".into(),
            message: format!("shortcut {shortcut} is not an allowed preset"),
        })
    }
}

fn parse(s: &str) -> Result<Shortcut, crate::error::AppError> {
    s.parse::<Shortcut>().map_err(|_| crate::error::AppError {
        code: "invalid_input".into(),
        message: format!("bad shortcut {s}"),
    })
}

/// Registers `settings.quick_shortcut` at startup; a refused grab only
/// warns — startup continues without the shortcut.
pub(crate) fn register_from_settings(app: &tauri::AppHandle) {
    let Some(chord) = crate::settings::load().quick_shortcut else {
        return;
    };
    if let Err(e) = apply(app, Some(&chord), None) {
        log_warn(&format!("quick shortcut unavailable: {}", e.message));
    }
}

fn log_warn(msg: &str) {
    use std::io::Write as _;
    drop(writeln!(std::io::stderr(), "{msg}"));
}

/// Switches the registered shortcut: registers the new one first, then
/// unregisters the old. `new` of `None` only unregisters. On failure the
/// old shortcut stays registered and the error is returned.
pub(crate) fn apply<RT: tauri::Runtime>(
    app: &tauri::AppHandle<RT>,
    new: Option<&str>,
    old: Option<&str>,
) -> Result<(), crate::error::AppError> {
    let gs = app.global_shortcut();
    if let Some(chord) = new {
        validate(chord)?;
        let sc = parse(chord)?;
        gs.on_shortcut(sc, move |h, _s, _e| {
            toggle(h);
        })
        .map_err(|e| crate::error::AppError {
            code: "shortcut_unavailable".into(),
            message: format!("cannot register {chord}: {e}"),
        })?;
    }
    if let Some(prev) = old
        && let Ok(sc) = prev.parse::<Shortcut>()
    {
        drop(gs.unregister(sc));
    }
    Ok(())
}

/// Shown → hide, hidden → show.
fn toggle<RT: tauri::Runtime>(app: &tauri::AppHandle<RT>) {
    use tauri::Manager as _;
    let visible = app
        .get_webview_window("quick")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if visible {
        crate::quick::hide(app);
    } else {
        crate::quick::show_quick(app);
    }
}
