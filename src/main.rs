use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fallow-luau", about = "Fallow-shaped codebase intelligence for Luau")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the capability manifest (commands and planned analyses).
    Schema,
}

fn main() {
    match Cli::parse().command {
        Command::Schema => {
            let schema = serde_json::json!({
                "name": "fallow-luau",
                "commands": ["schema", "health", "dead-code", "dupes", "audit"],
                "status": "bootstrap",
            });
            println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        }
    }
}
