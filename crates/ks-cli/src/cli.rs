//! Command-line surface.

use clap::{Parser, Subcommand};

/// Personal local secret vault with an age-compatible on-disk format.
#[derive(Debug, Parser)]
#[command(name = "ks", version, about, disable_help_subcommand = true)]
pub(crate) struct Cli {
    /// Vault directory (env: `KS_VAULT`; default `~/.ks`).
    #[arg(long, global = true)]
    pub(crate) vault: Option<String>,
    /// Non-interactive machine-readable output: exactly one JSON object.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// Read the master passphrase from this file (env: `KS_PASSPHRASE_FILE`).
    #[arg(long, global = true)]
    pub(crate) passphrase_file: Option<String>,
    /// Skip the session agent when unlocking.
    #[arg(long, global = true)]
    pub(crate) no_agent: bool,
    #[command(subcommand)]
    /// The subcommand to run.
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
/// Subcommands.
pub(crate) enum Command {
    /// Create a new vault.
    Init,
    /// List entries (no unlock needed).
    Ls {
        /// Optional name prefix (segment-wise).
        prefix: Option<String>,
    },
    /// Show an entry.
    Show {
        /// Entry name.
        name: String,
        /// Print only this field.
        #[arg(short = 'f')]
        field: Option<String>,
        /// Copy to the clipboard instead of printing.
        #[arg(short = 'c')]
        copy: bool,
    },
    /// Store a secret: one line (default), -m multiline from stdin, -b binary.
    Set {
        /// Entry name.
        name: String,
        /// Read multiple lines from stdin until EOF.
        #[arg(short = 'm')]
        multiline: bool,
        /// Read raw bytes from stdin.
        #[arg(short = 'b')]
        binary: bool,
        /// Overwrite an existing entry without asking.
        #[arg(long)]
        force: bool,
    },
    /// Edit a text entry in $VISUAL/$EDITOR/vi.
    Edit {
        /// Entry name (created if absent).
        name: String,
    },
    /// Generate a random password.
    Gen {
        /// Entry name to store under (omit to just print).
        name: Option<String>,
        /// Length, 1-1024 (default 32).
        #[arg(short = 'l', default_value_t = 32)]
        length: usize,
        /// Character set: alphanum|hex|printable|slug.
        #[arg(short = 's', default_value = "alphanum")]
        set: String,
        /// Copy to the clipboard instead of printing.
        #[arg(short = 'c')]
        copy: bool,
        /// Overwrite an existing entry without asking.
        #[arg(long)]
        force: bool,
    },
    /// Remove an entry.
    Rm {
        /// Entry name.
        name: String,
        /// Remove without asking.
        #[arg(long)]
        force: bool,
    },
    /// Rename an entry.
    Mv {
        /// Current name.
        from: String,
        /// New name.
        to: String,
        /// Overwrite the destination without asking.
        #[arg(long)]
        force: bool,
    },
    /// Show a TOTP code and its remaining validity.
    Otp {
        /// Entry name.
        name: String,
        /// Copy the code to the clipboard.
        #[arg(short = 'c')]
        copy: bool,
    },
    /// Run a command with secrets injected as environment variables.
    Run {
        /// VAR=name[#field] mappings.
        #[arg(short = 'e', value_name = "VAR=name[#field]")]
        env: Vec<String>,
        /// Command and arguments to run.
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    /// Change the master passphrase. K stays the same; this is not a
    /// revocation measure — use key rotation (`ks rekey`, later) for that.
    Passwd,
    /// Check the vault for problems.
    Doctor {
        /// Remove temp files and fix permissions.
        #[arg(long)]
        fix: bool,
    },
    /// Internal: clipboard helper process.
    #[command(hide = true)]
    ClipboardDaemon {
        /// Seconds before clearing.
        secs: u64,
    },
}

impl Cli {
    /// Resolves the vault path: `--vault` > `KS_VAULT` > `~/.ks`.
    ///
    /// # Errors
    ///
    /// [`ks::Error::InvalidInput`] when the home directory cannot be found.
    pub(crate) fn vault_path(&self) -> ks::Result<std::path::PathBuf> {
        self.vault
            .as_ref()
            .map_or_else(ks::Vault::default_path, |v| Ok(std::path::PathBuf::from(v)))
    }
}
