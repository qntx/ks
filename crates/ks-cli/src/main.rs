//! `ks` — personal local secret vault (work in progress towards 0.8.0).

use clap::Parser;

/// Personal local secret vault with an age-compatible on-disk format.
#[derive(Parser)]
#[command(name = "ks", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
