use clap::{Parser, Subcommand};
use zeroize::Zeroizing;

pub mod del;
pub mod env;
pub mod find;
pub mod generate;
pub mod info;
pub mod init;
pub mod io;
pub mod list;
pub mod lock;
pub mod get;
pub mod passwd;
pub mod set;

#[derive(Parser)]
#[command(
    name = "ks",
    version,
    about = "Key Store — secure local secret manager",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new vault with a master passphrase.
    Init,

    /// Print a secret value to stdout (pipe-friendly).
    Get {
        /// Secret path, e.g. `github/token`.
        path: String,
        /// Copy to clipboard instead of printing.
        #[arg(short, long)]
        copy: bool,
    },

    /// Store or update a secret (prompts for value).
    Set {
        /// Secret path, e.g. `github/token`.
        path: String,
        /// Optional description note.
        #[arg(short, long)]
        note: Option<String>,
        /// Overwrite if the path already exists without prompting.
        #[arg(short, long)]
        force: bool,
    },

    /// Delete a secret.
    #[command(alias = "rm")]
    Del {
        /// Secret path to delete.
        path: String,
        /// Skip confirmation prompt.
        #[arg(short, long)]
        force: bool,
    },

    /// List secrets in tree format.
    #[command(alias = "list")]
    Ls {
        /// Only list secrets under this prefix.
        #[arg(default_value = "")]
        prefix: String,
    },

    /// Generate and optionally store a random secret.
    Gen {
        /// Path to store the secret (prints only if omitted).
        path: Option<String>,
        /// Length in characters.
        #[arg(short, long, default_value = "32")]
        length: usize,
        /// Character set: `alphanum`, `hex`, `printable`.
        #[arg(short = 's', long, default_value = "alphanum")]
        charset: String,
        /// Overwrite existing secret at path without prompting.
        #[arg(short, long)]
        force: bool,
    },

    /// Print secrets as shell export statements.
    Env {
        /// Paths to include (all if empty).
        paths: Vec<String>,
        /// Shell dialect: `bash`, `fish`, `powershell`.
        #[arg(long, default_value = "bash")]
        shell: String,
    },

    /// Search secrets by path or note (case-insensitive).
    Find {
        /// Search query string.
        query: String,
    },

    /// Show metadata for a secret.
    Info {
        /// Secret path.
        path: String,
    },

    /// Change the master passphrase.
    Passwd,

    /// Export all secrets as JSON (plaintext — handle carefully).
    Export {
        /// Write to file instead of stdout.
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Import secrets from a JSON or .env file.
    Import {
        /// Path to file (stdin if omitted).
        file: Option<String>,
        /// Treat input as a .env (KEY=VALUE) file.
        #[arg(long)]
        dotenv: bool,
    },

    /// Lock the vault by clearing the session cache.
    Lock,
}

/// Opens the vault, trying (in order): `KS_PASSPHRASE` env, OS keyring, interactive prompt.
///
/// # Errors
/// Returns [`ks::Error::VaultNotFound`] if no vault exists, or
/// [`ks::Error::WrongPassphrase`] if the passphrase is incorrect.
pub fn open_vault(config: &ks::Config) -> ks::Result<ks::Vault> {
    if let Ok(raw) = std::env::var("KS_PASSPHRASE") {
        return ks::Vault::open(config, Zeroizing::new(raw));
    }

    if let Some(cached) = ks::session::get_passphrase() {
        match ks::Vault::open(config, cached) {
            Ok(v) => return Ok(v),
            Err(ks::Error::WrongPassphrase) => {}
            Err(e) => return Err(e),
        }
    }

    let raw: String = cliclack::password("Enter master passphrase")
        .mask('•')
        .interact()
        .map_err(|e| ks::Error::Io(std::io::Error::other(e.to_string())))?;

    let passphrase = Zeroizing::new(raw);
    let vault = ks::Vault::open(config, passphrase.clone())?;
    let _ = ks::session::set_passphrase(&passphrase);
    Ok(vault)
}
