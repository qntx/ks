//! Command-line surface.

use clap::{Parser, Subcommand, ValueEnum};

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
    Init {
        /// Also create a git repository and commit.
        #[arg(long, conflicts_with = "clone")]
        git: bool,
        /// Clone the vault from this URL instead of creating it.
        #[arg(long)]
        clone: Option<String>,
    },
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
    /// Commit, pull --rebase and push the vault (no unlock).
    Sync {
        /// On conflict keep this side.
        #[arg(long, value_enum)]
        keep: Option<KeepSide>,
    },
    /// Unlock the vault key into the session agent.
    Unlock {
        /// Idle timeout (e.g. 15m, 300s, 1h).
        #[arg(long, default_value = "15m")]
        ttl: String,
        /// Maximum lifetime (e.g. 8h, max 24h).
        #[arg(long, default_value = "8h")]
        max: String,
    },
    /// Stop the session agent for this vault.
    Lock,
    /// Show vault, agent and git status (no unlock).
    Status,
    /// Session agent daemon (internal, spawned by `ks unlock`).
    #[command(name = "__agent", hide = true)]
    Agent {
        /// Idle timeout in seconds.
        #[arg(long)]
        ttl: u64,
        /// Maximum lifetime in seconds.
        #[arg(long)]
        max: u64,
    },
    /// Run `git` inside the vault, passing all arguments through.
    Git {
        /// Arguments for git.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        args: Vec<String>,
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

/// Which side `ks sync --keep` retains on a conflict.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum KeepSide {
    /// The local commits being replayed.
    Local,
    /// The upstream (remote) version.
    Remote,
}
