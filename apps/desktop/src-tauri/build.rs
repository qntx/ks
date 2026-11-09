//! Tauri build step: config validation, capability/codegen (declaring
//! every app command enables per-window capability checks), and asset
//! embedding.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_state",
            "recent_vaults",
            "open_vault",
            "create_vault",
            "clone_vault",
            "passphrase_strength",
            "unlock",
            "unlock_device",
            "devices",
            "set_device",
            "lock",
            "heartbeat",
            "list_entries",
            "entry_detail",
            "reveal_field",
            "copy_field",
            "totp_code",
            "edit_entry",
            "save_entry",
            "rename_entry",
            "delete_entry",
            "import_binary",
            "export_binary",
            "generate_password",
            "get_settings",
            "set_settings",
            "vault_status",
            "doctor",
            "doctor_fix",
            "rotation_pending",
            "change_passphrase",
            "rotate_key",
            "autostart",
            "set_autostart",
            "set_tray_labels",
            "hide_quick",
            "git_status",
            "sync_now",
            "sync_state",
        ]),
    ))?;
    Ok(())
}
