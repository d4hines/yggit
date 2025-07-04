use clap::Parser;
use clap::Subcommand;
use commands::push::Push;
use commands::show::Show;
use git::Git;

mod commands;
mod core;
mod errors;
mod git;
mod github;
mod parser;

#[derive(Debug, Parser)] // requires `derive` feature
#[command(name = "git")]
#[command(about = "A fictional versioning CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Push(Push),
    Show(Show),
}

fn main() {
    env_logger::init();
    
    let args = Cli::parse();

    let git = Git::open(".");

    let result = match args.command {
        Commands::Push(push) => push.execute(git),
        Commands::Show(show) => show.execute(git),
    };
    
    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
