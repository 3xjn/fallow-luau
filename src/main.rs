use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use fallow_luau::{
    analyze_audit, analyze_dead_code, analyze_dupes, analyze_flags, analyze_project,
    build_require_graph, collect_suppressions, discover_files, explain_rule, init_config,
    inspect_target, list_report, load_config, render_saved_report, run_mcp_stdio, schema_manifest,
    trace_symbol, watch_loop, write_viz, AuditOptions, DeadCodeOptions, DupesOptions, HealthOptions,
    InitFormat, ProjectOptions, ReportFormat,
};

#[derive(Parser)]
#[command(name = "fallow-luau", about = "Fallow-shaped codebase intelligence for Luau")]
struct Cli {
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    #[arg(long, global = true)]
    explain: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Schema,
    List,
    Health {
        #[arg(long, default_value_t = false)]
        complexity: bool,
        #[arg(long, default_value_t = false)]
        file_scores: bool,
        #[arg(long, default_value_t = false)]
        hotspots: bool,
        #[arg(long, default_value_t = false)]
        targets: bool,
        #[arg(long, default_value_t = false)]
        score: bool,
        #[arg(long, default_value_t = 20)]
        max_cyclomatic: u32,
        #[arg(long, default_value_t = 15)]
        max_cognitive: u32,
        #[arg(long, default_value_t = 60)]
        max_unit_size: usize,
        #[arg(long, default_value_t = 180.0)]
        since_days: f64,
        #[arg(long)]
        baseline: Option<PathBuf>,
        #[arg(long)]
        save_baseline: Option<PathBuf>,
        #[arg(long, short = 'f', default_value = "json")]
        format: OutputFormat,
    },
    #[command(name = "dead-code")]
    DeadCode {
        #[arg(long = "entry")]
        entries: Vec<String>,
    },
    Dupes {
        #[arg(long, default_value_t = 30)]
        min_tokens: usize,
        #[arg(long, default_value_t = 5)]
        min_lines: usize,
    },
    Audit {
        #[arg(long)]
        changed_since: Option<String>,
    },
    Explain { id: String },
    Inspect {
        /// File path (relative to --root)
        path: String,
        /// Optional returned key / function name
        #[arg(long)]
        symbol: Option<String>,
    },
    Trace {
        path: String,
        #[arg(long)]
        key: Option<String>,
        #[arg(long, default_value_t = 3)]
        depth: usize,
    },
    Watch {
        #[arg(long, default_value_t = 1000)]
        interval_ms: u64,
        #[arg(long)]
        changed_since: Option<String>,
    },
    Init {
        #[arg(long, value_enum, default_value = "json")]
        format: InitCliFormat,
    },
    Config,
    Suppressions,
    Report {
        /// Saved --format json results file
        path: PathBuf,
        #[arg(long, value_enum, default_value = "markdown")]
        format: ReportCliFormat,
    },
    Flags,
    Viz {
        #[arg(long, default_value = "fallow-luau-viz.html")]
        out: PathBuf,
    },
    Mcp,
}

#[derive(Clone, ValueEnum)]
enum OutputFormat {
    Json,
}

#[derive(Clone, ValueEnum)]
enum InitCliFormat {
    Json,
    Toml,
}

#[derive(Clone, ValueEnum)]
enum ReportCliFormat {
    Json,
    Compact,
    Markdown,
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
            let root = canon(&cli.root)?;
            let files = discover_files(&root);
            let graph = build_require_graph(&root, &files)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&list_report(
                    &root.display().to_string(),
                    &graph,
                    cli.explain
                ))
                .unwrap()
            );
            Ok(())
        }
        Command::Health {
            complexity,
            file_scores,
            hotspots,
            targets,
            score,
            max_cyclomatic,
            max_cognitive,
            max_unit_size,
            since_days,
            baseline,
            save_baseline,
            format: _,
        } => {
            let any = complexity || file_scores || hotspots || targets || score;
            let mut health = HealthOptions {
                max_cyclomatic,
                max_cognitive,
                max_unit_size,
                explain: cli.explain,
                since_days,
                score: if any { score } else { true },
                baseline: baseline.clone(),
                save_baseline: save_baseline.clone(),
                ..HealthOptions::default()
            };
            if any {
                health.complexity = complexity || (!file_scores && !hotspots && !targets && score);
                health.file_scores = file_scores || (!complexity && !hotspots && !targets);
                health.hotspots = hotspots;
                health.targets = targets || save_baseline.is_some() || baseline.is_some();
                if !complexity && !file_scores && !hotspots && !targets && score {
                    health.complexity = true;
                    health.file_scores = true;
                }
            }
            let root = canon(&cli.root)?;
            let files = discover_files(&root);
            let graph = build_require_graph(&root, &files)?;
            let dead = analyze_dead_code(&root, &files, &graph, &DeadCodeOptions::default())?;
            health.dead_ratio_override = Some(dead.dead_ratio_by_file.clone());
            health.dead_file_count = Some(dead.unused_files.len());
            health.dead_export_count = Some(dead.unused_exports.len());
            health.total_export_count = Some(
                dead.dead_ratio_by_file
                    .keys()
                    .count()
                    .max(dead.unused_exports.len()),
            );
            health.circular_deps = Some(dead.cycles.len());
            let dupes = analyze_dupes(&root, &files, &DupesOptions::default())?;
            health.duplication_pct = Some(dupes.stats.duplication_percentage);

            let (_project, report) =
                analyze_project(&cli.root, &ProjectOptions { health })?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::DeadCode { entries } => {
            let root = canon(&cli.root)?;
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
            let root = canon(&cli.root)?;
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
        Command::Inspect { path, symbol } => {
            let report = inspect_target(
                &cli.root,
                &path,
                symbol.as_deref(),
                cli.explain,
            )?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::Trace { path, key, depth } => {
            let report = trace_symbol(&cli.root, &path, key.as_deref(), depth, cli.explain)?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::Watch {
            interval_ms,
            changed_since,
        } => watch_loop(&cli.root, interval_ms, changed_since, cli.explain),
        Command::Init { format } => {
            let root = canon(&cli.root)?;
            let fmt = match format {
                InitCliFormat::Json => InitFormat::Json,
                InitCliFormat::Toml => InitFormat::Toml,
            };
            let path = init_config(&root, fmt)?;
            println!(
                "{}",
                serde_json::json!({ "wrote": path.display().to_string() })
            );
            Ok(())
        }
        Command::Config => {
            let root = canon(&cli.root)?;
            let resolved = load_config(&root)?;
            let mut v = serde_json::to_value(&resolved).unwrap();
            if cli.explain {
                v.as_object_mut().unwrap().insert(
                    "_meta".into(),
                    serde_json::json!({
                        "docs": "https://docs.fallow.tools/cli/config",
                        "files": [".fallow-luau.json", "fallow-luau.toml"]
                    }),
                );
            }
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            Ok(())
        }
        Command::Suppressions => {
            let report = collect_suppressions(&cli.root, cli.explain)?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::Report { path, format } => {
            let fmt = match format {
                ReportCliFormat::Json => ReportFormat::Json,
                ReportCliFormat::Compact => ReportFormat::Compact,
                ReportCliFormat::Markdown => ReportFormat::Markdown,
            };
            print!("{}", render_saved_report(&path, fmt)?);
            Ok(())
        }
        Command::Flags => {
            let report = analyze_flags(&cli.root, cli.explain)?;
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            Ok(())
        }
        Command::Viz { out } => {
            let out_path = if out.is_absolute() {
                out
            } else {
                cli.root.join(out)
            };
            write_viz(&cli.root, &out_path)?;
            println!(
                "{}",
                serde_json::json!({ "wrote": out_path.display().to_string() })
            );
            Ok(())
        }
        Command::Mcp => run_mcp_stdio(),
    }
}

fn canon(root: &PathBuf) -> Result<PathBuf, String> {
    root.canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))
}
