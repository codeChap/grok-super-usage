mod billing;
mod grok;
mod proto;
mod scan;
mod util;

pub use clap::Parser;

use std::path::PathBuf;

use clap::Subcommand;

/// SuperGrok weekly usage and xAI API invoice spend for the Omarchy bar.
#[derive(Debug, Parser)]
#[command(name = "grok-super-usage", version, about)]
pub struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// SuperGrok weekly pool from grok.com GetGrokCreditsConfig.
    Grok {
        /// Print present/absent/unreadable if a Grok token exists (no usage API).
        #[arg(long)]
        probe: bool,
        /// Path to Grok auth.json (default: ~/.grok/auth.json).
        #[arg(long, env = "GROK_AUTH_PATH")]
        auth: Option<PathBuf>,
    },
    /// Current-cycle xAI API postpaid spend (Management API invoice preview).
    Billing {
        /// Print present/absent/unreadable if a key file or env var exists.
        #[arg(long)]
        probe: bool,
        /// Path to a file containing the management key (not the key itself).
        #[arg(long = "key-file")]
        key_file: Option<PathBuf>,
    },
    /// Write a management key from stdin to a chmod 600 file and print its path.
    StoreKey {
        /// Destination file (default: plugin management.key).
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

impl Cli {
    pub fn run(self) -> i32 {
        match self.cmd {
            Cmd::Grok { probe, auth } => grok::run(probe, auth),
            Cmd::Billing { probe, key_file } => billing::run(probe, key_file),
            Cmd::StoreKey { out } => billing::store_key(out),
        }
    }
}
