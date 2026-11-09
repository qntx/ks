//! Desktop shell for the ks secret vault.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[allow(
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    reason = "command glue: Tauri signatures, doc style and nesting follow framework conventions"
)]
mod commands;
#[cfg(test)]
mod desk_tests;
#[allow(
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    reason = "thin error adapter"
)]
mod error;
#[allow(
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    reason = "timer/platform glue"
)]
mod lock;
#[allow(clippy::restriction, clippy::pedantic, reason = "secret marker type")]
mod secret;
#[allow(
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    reason = "persistence glue"
)]
mod settings;
#[allow(
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    reason = "state glue"
)]
mod state;
mod sync;
mod tray;
#[allow(
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    reason = "notify glue"
)]
mod watch;
mod window;

use std::sync::Arc;

use tauri::Manager as _;

/// The specta builder is shared between `invoke_handler` and the
/// bindings test.
#[expect(
    trivial_casts,
    reason = "collect_commands! emits a redundant fn-pointer cast"
)]
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    use commands::{
        __cmd__app_state, __cmd__clone_vault, __cmd__copy_field, __cmd__create_vault,
        __cmd__delete_entry, __cmd__devices, __cmd__edit_entry, __cmd__entry_detail,
        __cmd__export_binary, __cmd__generate_password, __cmd__get_settings, __cmd__heartbeat,
        __cmd__import_binary, __cmd__list_entries, __cmd__lock, __cmd__open_vault,
        __cmd__passphrase_strength, __cmd__recent_vaults, __cmd__rename_entry, __cmd__reveal_field,
        __cmd__save_entry, __cmd__set_device, __cmd__set_settings, __cmd__totp_code, __cmd__unlock,
        __cmd__unlock_device, __cmd__vault_status, __specta__fn__app_state,
        __specta__fn__autostart, __specta__fn__change_passphrase, __specta__fn__clone_vault,
        __specta__fn__copy_field, __specta__fn__create_vault, __specta__fn__delete_entry,
        __specta__fn__devices, __specta__fn__doctor, __specta__fn__doctor_fix,
        __specta__fn__edit_entry, __specta__fn__entry_detail, __specta__fn__export_binary,
        __specta__fn__generate_password, __specta__fn__get_settings, __specta__fn__heartbeat,
        __specta__fn__import_binary, __specta__fn__list_entries, __specta__fn__lock,
        __specta__fn__open_vault, __specta__fn__passphrase_strength, __specta__fn__recent_vaults,
        __specta__fn__rename_entry, __specta__fn__reveal_field, __specta__fn__rotate_key,
        __specta__fn__rotation_pending, __specta__fn__save_entry, __specta__fn__set_autostart,
        __specta__fn__set_device, __specta__fn__set_settings, __specta__fn__set_tray_labels,
        __specta__fn__totp_code, __specta__fn__unlock, __specta__fn__unlock_device,
        __specta__fn__vault_status, __tauri_command_name_app_state, __tauri_command_name_autostart,
        __tauri_command_name_change_passphrase, __tauri_command_name_clone_vault,
        __tauri_command_name_copy_field, __tauri_command_name_create_vault,
        __tauri_command_name_delete_entry, __tauri_command_name_devices,
        __tauri_command_name_doctor, __tauri_command_name_doctor_fix,
        __tauri_command_name_edit_entry, __tauri_command_name_entry_detail,
        __tauri_command_name_export_binary, __tauri_command_name_generate_password,
        __tauri_command_name_get_settings, __tauri_command_name_heartbeat,
        __tauri_command_name_import_binary, __tauri_command_name_list_entries,
        __tauri_command_name_lock, __tauri_command_name_open_vault,
        __tauri_command_name_passphrase_strength, __tauri_command_name_recent_vaults,
        __tauri_command_name_rename_entry, __tauri_command_name_reveal_field,
        __tauri_command_name_rotate_key, __tauri_command_name_rotation_pending,
        __tauri_command_name_save_entry, __tauri_command_name_set_autostart,
        __tauri_command_name_set_device, __tauri_command_name_set_settings,
        __tauri_command_name_set_tray_labels, __tauri_command_name_totp_code,
        __tauri_command_name_unlock, __tauri_command_name_unlock_device,
        __tauri_command_name_vault_status, app_state, autostart, change_passphrase, clone_vault,
        copy_field, create_vault, delete_entry, devices, doctor, doctor_fix, edit_entry,
        entry_detail, export_binary, generate_password, get_settings, heartbeat, import_binary,
        list_entries, lock, open_vault, passphrase_strength, recent_vaults, rename_entry,
        reveal_field, rotate_key, rotation_pending, save_entry, set_autostart, set_device,
        set_settings, set_tray_labels, totp_code, unlock, unlock_device, vault_status,
    };
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            app_state,
            autostart::<tauri::Wry>,
            set_autostart::<tauri::Wry>,
            set_tray_labels::<tauri::Wry>,
            recent_vaults,
            open_vault::<tauri::Wry>,
            create_vault::<tauri::Wry>,
            clone_vault::<tauri::Wry>,
            passphrase_strength,
            unlock::<tauri::Wry>,
            unlock_device::<tauri::Wry>,
            devices,
            set_device,
            sync::git_status,
            sync::sync_now,
            sync::sync_state,
            lock,
            change_passphrase::<tauri::Wry>,
            rotate_key::<tauri::Wry>,
            rotation_pending,
            doctor,
            doctor_fix,
            heartbeat,
            list_entries,
            entry_detail,
            reveal_field,
            copy_field,
            totp_code,
            edit_entry,
            save_entry,
            rename_entry,
            delete_entry,
            import_binary,
            export_binary,
            generate_password,
            get_settings,
            set_settings,
            vault_status,
        ])
        .events(tauri_specta::collect_events![
            lock::Locked,
            sync::SyncState,
            watch::EntriesChanged
        ])
}

#[expect(
    clippy::disallowed_methods,
    reason = "tauri::generate_context! calls std::process::exit on a malformed config"
)]
fn main() -> tauri::Result<()> {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all()
                        & !tauri_plugin_window_state::StateFlags::VISIBLE,
                )
                .with_denylist(&["quick"])
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(state::AppState::new()))
        .manage(tray::Tray::default())
        .invoke_handler(specta_builder().invoke_handler())
        .setup(|app| {
            lock::init(app.handle());
            let state = Arc::clone(app.state::<Arc<state::AppState>>().inner());
            lock::watch_system(Arc::clone(&state), settings::load().lock_on_sleep);
            // Auto-open the last-used vault, else the default one (still locked).
            for path in [settings::last_vault(), ks::Vault::default_path().ok()]
                .into_iter()
                .flatten()
            {
                if let Ok(v) = ks::Vault::open(&path) {
                    drop(commands::install_open(&state, v));
                    tray::refresh(app.handle());
                    break;
                }
            }
            tray::build(app)?;
            // `--autostart` leaves the window hidden (tray only).
            if !std::env::args().any(|a| a == "--autostart") {
                window::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e
                && w.label() == "main"
            {
                window::hide_main(w.app_handle());
                api.prevent_close();
            }
        })
        .build(tauri::generate_context!())
        .map(|app| {
            app.run(|handle, event| {
                match event {
                    tauri::RunEvent::ExitRequested { api, .. } => {
                        on_exit_requested(handle, &api);
                    }
                    // macOS Dock/Finder relaunch — app bundles are already
                    // single-instance through LaunchServices.
                    tauri::RunEvent::Reopen { .. } => window::show_main(handle),
                    _ => {}
                }
            });
        })
}

/// One-shot exit gate: the first `ExitRequested` is prevented so a
/// thread can run `sync::quit` (lock + bounded sync wait); the second —
/// issued by that thread — proceeds.
fn on_exit_requested(handle: &tauri::AppHandle, api: &tauri::ExitRequestApi) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static EXITING: AtomicBool = AtomicBool::new(false);
    if EXITING.swap(true, Ordering::SeqCst) {
        return;
    }
    api.prevent_exit();
    let st = handle
        .try_state::<Arc<state::AppState>>()
        .map(|s| Arc::clone(s.inner()));
    let h = handle.clone();
    std::thread::spawn(move || {
        if let Some(st) = st {
            sync::quit(&st);
        }
        h.exit(0);
    });
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic,
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "test harness"
    )]

    use std::env;
    use std::fs;
    use std::process;

    use super::specta_builder;

    const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/ipc/bindings.gen.ts");

    fn render_bindings() -> String {
        let tmp = env::temp_dir().join(format!("ks-desktop-bindings-{}.ts", process::id()));
        specta_builder()
            .export(specta_typescript::Typescript::default(), &tmp)
            .expect("typescript bindings export failed");
        let rendered = fs::read_to_string(&tmp).expect("failed to read exported bindings");
        fs::remove_file(&tmp).expect("failed to remove temp bindings file");
        rendered
    }

    /// `bindings.gen.ts` is generated here and nowhere else: set
    /// `KS_UPDATE_BINDINGS=1` to rewrite it after changing a command.
    #[test]
    fn bindings_are_current() {
        let generated = render_bindings();
        if env::var("KS_UPDATE_BINDINGS").is_ok_and(|value| value == "1") {
            fs::write(BINDINGS_PATH, &generated).expect("failed to write bindings.gen.ts");
            return;
        }
        let committed = fs::read_to_string(BINDINGS_PATH)
            .expect("bindings.gen.ts is missing; regenerate with KS_UPDATE_BINDINGS=1");
        assert_eq!(
            generated, committed,
            "bindings.gen.ts is stale; regenerate with KS_UPDATE_BINDINGS=1"
        );
    }

    fn invoke_name(rest: &str) -> Option<String> {
        rest.find('\"').map(|end| rest[..end].to_owned())
    }

    /// Every command whose params or result mention `SecretText` must be
    /// in the allowlist (ADR-18).
    #[test]
    fn desk_core_secrets_allowlist() {
        let src = fs::read_to_string(BINDINGS_PATH).unwrap();
        let mut found: Vec<String> = Vec::new();
        for line in src.lines() {
            if !line.contains("SecretText") {
                continue;
            }
            if let Some(pos) = line.find("INVOKE")
                && let Some(p2) = line[pos..].find("(\"")
            {
                found.extend(invoke_name(&line[pos + p2 + 2..]));
            }
        }
        found.sort();
        let expected: Vec<&str> = vec![
            "change_passphrase",
            "clone_vault",
            "create_vault",
            "edit_entry",
            "generate_password",
            "passphrase_strength",
            "reveal_field",
            "rotate_key",
            "save_entry",
            "unlock",
        ];
        assert_eq!(found, expected, "SecretText commands drifted");
    }
}
