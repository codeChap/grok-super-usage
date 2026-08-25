use codechap_grokbar::{Cli, Parser};

fn main() {
    let cli = Cli::parse();
    std::process::exit(cli.run());
}
