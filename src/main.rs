use grok_super_usage::{Cli, Parser};

fn main() {
    let cli = Cli::parse();
    std::process::exit(cli.run());
}
