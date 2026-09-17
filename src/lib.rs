mod accounts;
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
        /// Path to the live Grok auth.json (default: ~/.grok/auth.json).
        #[arg(long, env = "GROK_AUTH_PATH")]
        auth: Option<PathBuf>,
        /// Directory of extra saved auth.json snapshots (one SuperGrok login each).
        #[arg(long)]
        accounts_dir: Option<PathBuf>,
    },
    /// Copy the live Grok login into the accounts directory so it stays tracked after `grok login`.
    Snapshot {
        /// Path to Grok auth.json (default: ~/.grok/auth.json).
        #[arg(long, env = "GROK_AUTH_PATH")]
        auth: Option<PathBuf>,
        /// Destination directory (default: plugin accounts/).
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    /// Delete a saved auth snapshot. Refuses paths outside the accounts directory.
    Forget {
        /// Saved auth.json to remove.
        #[arg(long)]
        path: PathBuf,
        /// Accounts directory the path must sit inside.
        #[arg(long)]
        dir: Option<PathBuf>,
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
            Cmd::Grok {
                probe,
                auth,
                accounts_dir,
            } => grok::run(probe, auth, accounts_dir),
            Cmd::Snapshot { auth, dir } => grok::snapshot(auth, dir),
            Cmd::Forget { path, dir } => accounts::forget(path, dir),
            Cmd::Billing { probe, key_file } => billing::run(probe, key_file),
            Cmd::StoreKey { out } => billing::store_key(out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_lists_multi_account_commands() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("snapshot"));
        assert!(help.contains("forget"));
        let mut grok = Cli::command()
            .find_subcommand("grok")
            .expect("grok")
            .clone();
        let grok_help = grok.render_long_help().to_string();
        assert!(grok_help.contains("accounts-dir"));
    }
}
