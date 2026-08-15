use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use fallow_luau::{
    analyze_audit, analyze_dead_code, analyze_dupes, analyze_project, build_require_graph,
    discover_files, explain_rule, list_report, run_mcp_stdio, schema_manifest, AuditOptions,
    DeadCodeOptions, DupesOptions, HealthOptions, ProjectOptions,
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
        #[arg(long, default_value_t = false)]
        complexity: bool,
        #[arg(long, default_value_t = false)]
        file_scores: bool,
        #[arg(long, default_value_t = false)]
        hotspots: bool,
        #[arg(long, default_value_t = false)]
        targets: bool,
        #[arg(long, default_value_t = 20)]
        max_cyclomatic: u32,
        #[arg(long, default_value_t = 15)]
        max_cognitive: u32,
        #[arg(long, default_value_t = 60)]
        max_unit_size: usize,
        #[arg(long, default_value_t = 180.0)]
        since_days: f64,
        #[arg(long, short = 'f', default_value = "json")]
        format: OutputFormat,
    },
    /// Unused files, returned keys, locals, require cycles
    #[command(name = "dead-code")]
    DeadCode {
        /// Extra entry point (repeatable), relative to --root
        #[arg(long = "entry")]
        entries: Vec<String>,
    },
    /// Token / suffix-array clones
    Dupes {
        #[arg(long, default_value_t = 30)]
        min_tokens: usize,
        #[arg(long, default_value_t = 5)]
        min_lines: usize,
    },
    /// Combined dead-code + health + dupes
    Audit {
        /// Only findings in files changed since this git ref
        #[arg(long)]
        changed_since: Option<String>,
    },
    /// Print rule docs for one issue type
    Explain {
        /// Issue id, e.g. unused-file or high-cognitive-complexity
        id: String,
    },
    /// stdio MCP server (same library as CLI; always includes _meta)
    Mcp,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Json,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
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
            let any = complexity || file_scores || hotspots || targets;
            let mut health = HealthOptions {
                max_cyclomatic,
                max_cognitive,
                max_unit_size,
                explain: cli.explain,
                since_days,
                ..HealthOptions::default()
            };
            if any {
                health.complexity = complexity;
                health.file_scores = file_scores;
                health.hotspots = hotspots;
                health.targets = targets;
            }
            // Wire dead ratios into MI when possible.
            if let Ok(root) = cli.root.canonicalize() {
                let files = discover_files(&root);
                if let Ok(graph) = build_require_graph(&root, &files) {
                    if let Ok(dead) = analyze_dead_code(
                        &root,
                        &files,
                        &graph,
                        &DeadCodeOptions::default(),
                    ) {
                        health.dead_ratio_override = Some(dead.dead_ratio_by_file);
                    }
                }
            }
            let (_project, report) =
                analyze_project(&cli.root, &ProjectOptions { health })?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::DeadCode { entries } => {
            let root = cli
                .root
                .canonicalize()
                .map_err(|e| format!("canonicalize {}: {e}", cli.root.display()))?;
            let files = discover_files(&root);
            let graph = build_require_graph(&root, &files)?;
            let report = analyze_dead_code(
                &root,
                &files,
                &graph,
                &DeadCodeOptions {
                    explain: cli.explain,
                    extra_entries: entries,
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::Dupes {
            min_tokens,
            min_lines,
        } => {
            let root = cli
                .root
                .canonicalize()
                .map_err(|e| format!("canonicalize {}: {e}", cli.root.display()))?;
            let files = discover_files(&root);
            let report = analyze_dupes(
                &root,
                &files,
                &DupesOptions {
                    explain: cli.explain,
                    min_tokens,
                    min_lines,
                    ..DupesOptions::default()
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::Audit { changed_since } => {
            let report = analyze_audit(
                &cli.root,
                &AuditOptions {
                    explain: cli.explain,
                    changed_since,
                    ..AuditOptions::default()
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::Explain { id } => {
            println!("{}", serde_json::to_string_pretty(&explain_rule(&id)).unwrap());
            Ok(())
        }
        Command::Mcp => run_mcp_stdio(),
    }
}
