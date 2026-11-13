use clap::Parser;

mod cli;
mod process;
mod render;

fn main() {
    cli::Cli::parse();
}
