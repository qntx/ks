//! The `quick` floating window — a non-activating `NSPanel` on macOS,
//! a plain always-on-top undecorated window elsewhere. Created hidden
//! in setup; `show_quick` centers it under the cursor and emits
//! `quick-shown` so the frontend can focus its search field.

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(QuickPanel {
        config: {
            can_become_key_window: true,
            is_floating_panel: true,
        }
    })
}

/// Payload emitted to the `quick` window when it is shown.
#[derive(Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuickShown;

/// Creates the hidden `quick` window. Called once from setup.
pub(crate) fn create(app: &tauri::App) -> tauri::Result<()> {
    create_impl(app)
}

#[cfg(target_os = "macos")]
#[expect(
    clippy::unused_unit,
    reason = "panel_event! macro syntax requires `-> ()`"
)]
fn create_impl(app: &tauri::App) -> tauri::Result<()> {
    use tauri_nspanel::{CollectionBehavior, PanelBuilder, StyleMask, panel_event};

    let panel = PanelBuilder::<_, QuickPanel>::new(&app.handle().clone(), "quick")
        .with_window(|w| {
            w.inner_size(640.0, 420.0)
                .visible(false)
                .decorations(false)
                .content_protected(true)
        })
        .level(tauri_nspanel::PanelLevel::Status)
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary(),
        )
        .add_style_mask(StyleMask::empty().nonactivating_panel())
        .hides_on_deactivate(false)
        .build()?;
    // Hide when the panel resigns key — like Spotlight.
    panel_event!(ResignHandler {
        window_did_resign_key(notification: &objc2_foundation::NSNotification) -> (),
    });
    let handler = ResignHandler::new();
    let h = app.handle().clone();
    handler.window_did_resign_key(move |_| {
        if !pinned() {
            hide(&h);
        }
    });
    panel.set_event_handler(Some(handler.as_ref()));
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn create_impl(app: &tauri::App) -> tauri::Result<()> {
    tauri::WebviewWindowBuilder::new(app, "quick", tauri::WebviewUrl::default())
        .inner_size(640.0, 420.0)
        .visible(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .content_protected(true)
        .focused(false)
        .build()?;
    Ok(())
}

/// Shows the `quick` panel centered in the upper third of the monitor
/// under the cursor, makes it key, then emits `quick-shown`.
pub(crate) fn show_quick<RT: tauri::Runtime>(app: &tauri::AppHandle<RT>) {
    place_and_show(app);
    drop(tauri::Emitter::emit_to(
        app,
        "quick",
        "quick-shown",
        QuickShown,
    ));
}

#[cfg(target_os = "macos")]
fn place_and_show<RT: tauri::Runtime>(app: &tauri::AppHandle<RT>) {
    use tauri::Manager as _;
    use tauri_nspanel::ManagerExt as _;
    // The panel shows + takes key without activating the app (Spotlight
    // behaviour): Tauri's window focus path would activate ks.
    if let Some(w) = app.get_webview_window("quick") {
        center_upper(app, &w);
    }
    if let Ok(panel) = app.get_webview_panel("quick") {
        panel.show_and_make_key();
    }
}

#[cfg(not(target_os = "macos"))]
fn place_and_show<RT: tauri::Runtime>(app: &tauri::AppHandle<RT>) {
    use tauri::Manager as _;
    if let Some(w) = app.get_webview_window("quick") {
        center_upper(app, &w);
        drop(w.show());
        drop(w.set_focus());
    }
}

/// Centers `w` horizontally in the upper third of the monitor under the
/// cursor (falls back to the primary monitor).
fn center_upper<RT: tauri::Runtime>(app: &tauri::AppHandle<RT>, w: &tauri::WebviewWindow<RT>) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(m) = monitor
        && let Ok(win) = w.outer_size()
    {
        let pos = m.position();
        let size = m.size();
        let w32 = i32::try_from(win.width).unwrap_or(0);
        let mw = i32::try_from(size.width).unwrap_or(0);
        let mh = i32::try_from(size.height).unwrap_or(0);
        let x = pos.x + (mw - w32) / 2;
        let y = pos.y + mh / 4;
        drop(w.set_position(tauri::PhysicalPosition::new(x, y)));
    }
}

/// Hides the `quick` panel.
pub(crate) fn hide<RT: tauri::Runtime>(app: &tauri::AppHandle<RT>) {
    use tauri::Manager as _;
    #[cfg(target_os = "macos")]
    {
        use tauri_nspanel::ManagerExt as _;
        if let Ok(panel) = app.get_webview_panel("quick") {
            panel.hide();
            return;
        }
    }
    if let Some(w) = app.get_webview_window("quick") {
        drop(w.hide());
    }
}

/// Set while a quick-window device unlock is in flight: the Touch ID
/// prompt steals key status, and the resign-key handler must not hide
/// the panel mid-unlock.
static PIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn pinned() -> bool {
    PIN.load(std::sync::atomic::Ordering::Relaxed)
}

/// RAII guard: clears the pin on drop. `unlock_device` re-shows the
/// panel explicitly on completion.
pub(crate) struct Pin;

impl Pin {
    pub(crate) fn new() -> Self {
        PIN.store(true, std::sync::atomic::Ordering::Relaxed);
        Self
    }
}

impl Drop for Pin {
    fn drop(&mut self) {
        PIN.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
