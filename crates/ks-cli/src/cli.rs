//! Clap-derive command-line parser.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Modern, local-first secret manager built on the age encryption format.
#[derive(Debug, Parser)]
#[command(
    name = "ks",
    version,
    about,
    long_about = None,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// All top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new identity and store in the default location.
    Init {
        /// Initialise a git repository inside the store after creation.
        #[arg(long)]
        git: bool,
    },

    /// Print a secret value (defaults to the primary value).
    Get {
        /// Logical secret path (e.g. `github/token`).
        path: String,
        /// Copy the value to the clipboard instead of printing it.
        #[arg(short, long)]
        copy: bool,
        /// Print the named field instead of the primary value.
        #[arg(short = 'f', long)]
        field: Option<String>,
    },

    /// Store or update a secret (interactive masked prompt by default).
    Set {
        /// Logical secret path.
        path: String,
        /// Optional human-readable note.
        #[arg(short, long)]
        note: Option<String>,
        /// Overwrite an existing secret without prompting.
        #[arg(short, long)]
        force: bool,
        /// Treat the value as a TOTP source (otpauth URL or base32 secret).
        #[arg(long)]
        totp: bool,
    },

    /// Remove a secret.
    #[command(alias = "del")]
    Rm {
        /// Logical secret path.
        path: String,
        /// Skip the confirmation prompt.
        #[arg(short, long)]
        force: bool,
    },

    /// List secrets as a tree.
    #[command(alias = "list")]
    Ls {
        /// Only list paths under this prefix.
        #[arg(default_value = "")]
        prefix: String,
    },

    /// Rename or move a secret.
    Mv {
        /// Current logical path.
        from: String,
        /// New logical path.
        to: String,
    },

    /// Search secrets by path (and optionally by note).
    Find {
        /// Query string (case-insensitive substring match).
        query: String,
        /// Also scan decrypted notes (slow: requires decrypting every secret).
        #[arg(long)]
        notes: bool,
    },

    /// Show metadata for a secret (no values are printed).
    Info {
        /// Logical secret path.
        path: String,
    },

    /// Generate a random secret and optionally store it.
    Gen {
        /// Logical path to store the generated value (omit to print only).
        path: Option<String>,
        /// Character count.
        #[arg(short, long, default_value_t = 32)]
        length: usize,
        /// Character set: `alphanum`, `hex`, `printable`, `slug`.
        #[arg(short = 's', long, default_value = "alphanum")]
        charset: String,
        /// Overwrite an existing secret without prompting.
        #[arg(short, long)]
        force: bool,
        /// Copy the generated value to the clipboard.
        #[arg(short, long)]
        copy: bool,
    },

    /// Generate a TOTP code for a secret marked `kind = totp`.
    Otp {
        /// Logical path of a TOTP secret.
        path: String,
        /// Copy the code to the clipboard.
        #[arg(short, long)]
        copy: bool,
    },

    /// Run a command with secrets injected as environment variables.
    #[command(trailing_var_arg = true)]
    Run {
        /// Mapping `<path>=<ENV_NAME>` (repeatable).
        #[arg(short, long, value_name = "PATH=NAME")]
        env: Vec<String>,
        /// Inject every secret under this prefix using upper-cased
        /// `_`-joined names (repeatable).
        #[arg(short, long, value_name = "PREFIX")]
        prefix: Vec<String>,
        /// Command and arguments to execute.
        #[arg(required = true, allow_hyphen_values = true, trailing_var_arg = true)]
        cmd: Vec<String>,
    },

    /// Render a template, substituting `${KS:path}` markers with secret values.
    Inject {
        /// Input template file (`-` or omitted reads stdin).
        #[arg(short, long)]
        input: Option<PathBuf>,
        /// Output file (`-` or omitted writes stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Print shell `export` statements for the listed secrets (or all under a prefix).
    Env {
        /// Logical paths or prefixes.
        targets: Vec<String>,
        /// Output dialect: `sh`, `bash`, `fish`, `pwsh`.
        #[arg(long, default_value = "sh")]
        shell: String,
    },

    /// Manage the recipient public-key list.
    Recipients {
        #[command(subcommand)]
        cmd: RecipientsCmd,
    },

    /// Inspect or export the local identity.
    Identity {
        #[command(subcommand)]
        cmd: IdentityCmd,
    },

    /// Thin wrapper over the system `git` for syncing the store.
    Git {
        #[command(subcommand)]
        cmd: GitCmd,
    },

    /// Run a battery of health checks on the store and config.
    Doctor,

    /// Change the identity's passphrase.
    Passwd,

    /// Clear the cached session identity.
    Lock,

    /// Force unlock and cache the session identity.
    Unlock,
}

/// Subcommands of `ks recipients`.
#[derive(Debug, Subcommand)]
pub enum RecipientsCmd {
    /// List recipient public keys.
    #[command(alias = "list")]
    Ls,
    /// Add a recipient and re-encrypt every secret.
    Add {
        /// `age1…` public key.
        pubkey: String,
    },
    /// Remove a recipient and re-encrypt every secret.
    Rm {
        /// `age1…` public key.
        pubkey: String,
    },
}

/// Subcommands of `ks identity`.
#[derive(Debug, Subcommand)]
pub enum IdentityCmd {
    /// Print this device's public recipient (`age1…`).
    Show,
    /// Copy the encrypted identity file to `dest` (still passphrase-protected).
    Export {
        /// Destination file path.
        dest: PathBuf,
    },
}

/// Subcommands of `ks git`.
#[derive(Debug, Subcommand)]
pub enum GitCmd {
    /// Initialise a git repository inside the store directory.
    Init,
    /// `git add -A && git commit && git pull --rebase && git push`.
    Sync {
        /// Commit message (default: `ks: sync`).
        #[arg(short, long, default_value = "ks: sync")]
        message: String,
    },
    /// Show `git status -sb`.
    Status,
    /// Show the last N commits (`git log --oneline -nN`).
    Log {
        /// Number of commits.
        #[arg(short, long, default_value_t = 20)]
        n: usize,
    },
}
