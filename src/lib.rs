mod billing;
mod cursor;
mod grok;
mod proto;
mod scan;
mod util;

pub use clap::Parser;

use std::path::PathBuf;

use clap::Subcommand;

/// SuperGrok weekly usage (and optional Cursor monthly) for the Omarchy bar.
#[derive(Debug, Parser)]
#[command(name = "grokbar", version, about)]
pub struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// SuperGrok weekly pool from grok.com GetGrokCreditsConfig.
    Grok {
        /// Print ready/absent if a Grok token exists (no usage API).
        #[arg(long)]
        probe: bool,
        /// Path to Grok auth.json (default: ~/.grok/auth.json).
        #[arg(long, env = "GROK_AUTH_PATH")]
        auth: Option<PathBuf>,
    },
    /// Cursor monthly pools when the local Cursor session matches Grok.
    Cursor {
        /// Print ready/absent if a matching Cursor session exists.
        #[arg(long)]
        probe: bool,
        /// Path to Cursor CLI auth.json.
        #[arg(long, env = "CURSOR_AUTH_PATH")]
        auth: Option<PathBuf>,
        /// Path to Cursor state.vscdb (read-only).
        #[arg(long = "state-db", env = "CURSOR_STATE_DB")]
        state_db: Option<PathBuf>,
        /// Path to Grok auth.json used to match the account.
        #[arg(long = "grok-auth", env = "GROK_AUTH_PATH")]
        grok_auth: Option<PathBuf>,
    },
    /// Current-cycle xAI API postpaid spend (Management API invoice preview).
    Billing {
        /// Print ready/absent if a management key file or env var exists.
        #[arg(long)]
        probe: bool,
        /// Path to a file containing the management key (default: ~/dev/XAI-MGMT-KEY.txt).
        #[arg(long = "key-file", env = "XAI_MANAGEMENT_KEY_FILE")]
        key_file: Option<PathBuf>,
    },
}

impl Cli {
    pub fn run(self) -> i32 {
        match self.cmd {
            Cmd::Grok { probe, auth } => grok::run(probe, auth),
            Cmd::Cursor {
                probe,
                auth,
                state_db,
                grok_auth,
            } => cursor::run(probe, auth, state_db, grok_auth),
            Cmd::Billing { probe, key_file } => billing::run(probe, key_file),
        }
    }
}
