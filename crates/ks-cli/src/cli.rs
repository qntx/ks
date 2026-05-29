//! Clap-derive command-line parser.

use clap::{Parser, Subcommand};
use ks::pwgen::Charset;

/// Modern, local-first secret manager built on the age encryption format.
#[derive(Debug, Parser)]
#[command(name = "ks", version, about, long_about = None, arg_required_else_help = true)]
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

    /// List secrets as a tree.
    #[command(alias = "list")]
    Ls {
        /// Only list paths under this prefix.
        #[arg(default_value = "")]
        prefix: String,
    },

    /// Print a secret (whole file), or copy/extract part of it.
    #[command(alias = "get")]
    Show {
        /// Logical secret path (e.g. `github/token`).
        path: String,
        /// Copy the primary value (or `--field`) to the clipboard instead of printing.
        #[arg(short, long)]
        copy: bool,
        /// Operate on the named field instead of the whole secret.
        #[arg(short = 'f', long)]
        field: Option<String>,
        /// Print metadata (field names, never values) instead of the secret.
        #[arg(long)]
        meta: bool,
    },

    /// Store or update a secret (masked prompt, or stdin when piped).
    #[command(alias = "set")]
    Insert {
        /// Logical secret path.
        path: String,
        /// Read a multi-line secret until EOF instead of a single line.
        #[arg(short, long)]
        multiline: bool,
        /// Overwrite an existing secret without prompting.
        #[arg(short, long)]
        force: bool,
    },

    /// Edit a secret in `$EDITOR`.
    Edit {
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
        #[arg(short = 's', long, default_value = "alphanum", value_parser = parse_charset)]
        charset: Charset,
        /// Overwrite an existing secret without prompting.
        #[arg(short, long)]
        force: bool,
        /// Copy the generated value to the clipboard.
        #[arg(short, long)]
        copy: bool,
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

    /// Rename (move) a secret. Moves ciphertext only — no passphrase needed.
    Mv {
        /// Current logical path.
        from: String,
        /// New logical path.
        to: String,
    },

    /// Copy a secret. Copies ciphertext only — no passphrase needed.
    Cp {
        /// Source logical path.
        from: String,
        /// Destination logical path.
        to: String,
    },

    /// Search secrets by path, and optionally by decrypted content.
    #[command(alias = "find")]
    Grep {
        /// Query string (case-insensitive substring match).
        query: String,
        /// Also scan decrypted contents (slow: decrypts every secret).
        #[arg(long)]
        values: bool,
    },

    /// Generate a TOTP code from a secret containing an `otpauth://` source.
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
        /// Inject every secret under this prefix using upper-cased `_`-joined
        /// names (repeatable).
        #[arg(short, long, value_name = "PREFIX")]
        prefix: Vec<String>,
        /// Command and arguments to execute.
        #[arg(required = true, allow_hyphen_values = true, trailing_var_arg = true)]
        cmd: Vec<String>,
    },

    /// Manage the recipient public-key list.
    Recipients {
        #[command(subcommand)]
        cmd: RecipientsCmd,
    },

    /// Print this device's public recipient (`age1…`).
    Identity,

    /// Run git inside the store directory (passthrough): `ks git <args…>`.
    #[command(disable_help_flag = true)]
    Git {
        /// Arguments forwarded verbatim to the system `git`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run a battery of health checks on the store and config.
    Doctor,

    /// Change the identity's passphrase.
    Passwd,
}

/// Parses a `--charset` value into a [`Charset`], so a bad value is reported as
/// a clap usage error listing the accepted names.
fn parse_charset(raw: &str) -> Result<Charset, String> {
    raw.parse()
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
