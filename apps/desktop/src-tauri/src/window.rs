//! Main-window show/hide funnel — the activation policy follows
//! visibility on macOS: hidden means `Accessory` (no Dock icon), shown
//! means `Regular`. Every show and hide goes through these helpers.

use tauri::Manager as _;

/// Shows and focuses the main window (tray "Open ks", single-instance,
/// setup without `--autostart`). On macOS the app returns to `Regular`.
pub(crate) fn show_main<RT: tauri::Runtime>(handle: &tauri::AppHandle<RT>) {
    #[cfg(target_os = "macos")]
    drop(handle.set_activation_policy(tauri::ActivationPolicy::Regular));
    if let Some(w) = handle.get_webview_window("main") {
        drop(w.show());
        drop(w.set_focus());
    }
}

/// Hides the main window (`CloseRequested`). On macOS the app drops to
/// `Accessory` so the Dock icon and app switcher entry go away.
pub(crate) fn hide_main<RT: tauri::Runtime>(handle: &tauri::AppHandle<RT>) {
    if let Some(w) = handle.get_webview_window("main") {
        drop(w.hide());
    }
    #[cfg(target_os = "macos")]
    drop(handle.set_activation_policy(tauri::ActivationPolicy::Accessory));
}
