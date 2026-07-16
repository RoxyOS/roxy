mod cli;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Some(arg) = cli.arg {
        match arg {}
    }
}
