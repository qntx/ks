//! Desktop shell for the ks secret vault.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use commands::{__specta__fn__vault_status, vault_status};

/// The specta builder is shared between `invoke_handler` and the bindings
/// test, so the exported types always match what the app registers.
#[expect(
    trivial_casts,
    reason = "collect_commands! emits a redundant fn-pointer cast"
)]
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![vault_status])
}

#[expect(
    clippy::disallowed_methods,
    reason = "tauri::generate_context! calls std::process::exit on a malformed config"
)]
fn main() -> tauri::Result<()> {
    tauri::Builder::default()
        .invoke_handler(specta_builder().invoke_handler())
        .run(tauri::generate_context!())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic,
        reason = "test harness: panicking asserts"
    )]

    use std::env;
    use std::fs;
    use std::process;

    use super::specta_builder;

    const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.gen.ts");

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
}
