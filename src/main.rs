use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use fallow_luau::{
    analyze_project, build_require_graph, discover_files, list_report, schema_manifest,
    HealthOptions, ProjectOptions,
};

#[derive(Parser)]
#[command(name = "fallow-luau", about = "Fallow-shaped codebase intelligence for Luau")]
struct Cli {
    /// Project root (defaults to current directory)
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,

    /// Include `_meta` metric definitions in JSON
    #[arg(long, global = true)]
    explain: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the capability manifest
    Schema,
    /// List discovered files and the string-literal require graph
    List,
    /// Complexity, file scores, hotspots, and refactoring targets
    Health {
        /// Emit complexity findings
        #[arg(long, default_value_t = false)]
        complexity: bool,
        /// Emit per-file maintainability scores
        #[arg(long, default_value_t = false)]
        file_scores: bool,
        /// Emit churn × density hotspots
        #[arg(long, default_value_t = false)]
        hotspots: bool,
        /// Emit ranked refactoring targets
        #[arg(long, default_value_t = false)]
        targets: bool,
        /// Cyclomatic threshold (default 20)
        #[arg(long, default_value_t = 20)]
        max_cyclomatic: u32,
        /// Cognitive threshold (default 15)
        #[arg(long, default_value_t = 15)]
        max_cognitive: u32,
        /// Unit-size threshold for large_functions (default 60)
        #[arg(long, default_value_t = 60)]
        max_unit_size: usize,
        /// Git history window in days for hotspots (default 180 ≈ 6m)
        #[arg(long, default_value_t = 180.0)]
        since_days: f64,
        /// Output format
        #[arg(long, short = 'f', default_value = "json")]
        format: OutputFormat,
    },
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Json,
}

fn main() {
    let cli = Cli::parse();
    let result = run(cli);
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Schema => {
            println!(
                "{}",
                serde_json::to_string_pretty(&schema_manifest(cli.explain)).unwrap()
            );
            Ok(())
        }
        Command::List => {
            let root = cli
                .root
                .canonicalize()
                .map_err(|e| format!("canonicalize {}: {e}", cli.root.display()))?;
            let files = discover_files(&root);
            let graph = build_require_graph(&root, &files)?;
            let report = list_report(&root.display().to_string(), &graph, cli.explain);
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::Health {
            complexity,
            file_scores,
            hotspots,
            targets,
            max_cyclomatic,
            max_cognitive,
            max_unit_size,
            since_days,
            format: _,
        } => {
            // If no section flags, enable all standard sections.
            let any = complexity || file_scores || hotspots || targets;
            let mut health = HealthOptions {
                max_cyclomatic,
                max_cognitive,
                max_unit_size,
                explain: cli.explain,
                hotspots: if any { hotspots } else { true },
                targets: if any { targets } else { true },
                file_scores: if any { file_scores } else { true },
                complexity: if any { complexity } else { true },
                since_days,
                ..HealthOptions::default()
            };
            // When only some flags set, still respect them.
            if any {
                health.complexity = complexity;
                health.file_scores = file_scores;
                health.hotspots = hotspots;
                health.targets = targets;
            }
            let (_project, report) = analyze_project(
                &cli.root,
                &ProjectOptions { health },
            )?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
    }
}
