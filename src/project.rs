use std::path::{Path, PathBuf};

use full_moon::parse;
use serde::Serialize;

use crate::complexity::analyze_functions;
use crate::discover::{discover_configured_files, is_test_path};
use crate::graph::{build_require_graph, display_rel, RequireGraph};
use crate::health::{analyze_health, FileAnalysis, HealthOptions, HealthReport};

#[derive(Debug, Clone, Default)]
pub struct ProjectOptions {
    pub health: HealthOptions,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub root: PathBuf,
    pub files: Vec<PathBuf>,
    pub graph: RequireGraph,
}

pub fn analyze_project(
    root: &Path,
    opts: &ProjectOptions,
) -> Result<(Project, HealthReport), String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    let files = discover_configured_files(&root)?;
    let graph = build_require_graph(&root, &files)?;

    let mut analyses = Vec::new();
    for file in &files {
        let source =
            std::fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
        let lines = source.lines().count();
        let path = display_rel(&root, file);
        let ast = parse(&source).map_err(|errs| {
            format!(
                "parse {}: {}",
                file.display(),
                errs.iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        })?;
        let functions = analyze_functions(&ast);
        analyses.push(FileAnalysis {
            is_test: is_test_path(Path::new(&path)),
            path,
            abs: file.clone(),
            lines,
            functions,
        });
    }

    // Both CLI and MCP use this composition so dead-code and duplication inputs
    // cannot silently disappear from the same health report on one surface.
    let mut health = opts.health.clone();
    let dead = crate::dead_code::analyze_dead_code(
        &root,
        &files,
        &graph,
        &crate::dead_code::DeadCodeOptions::default(),
    )?;
    health
        .dead_ratio_override
        .get_or_insert(dead.dead_ratio_by_file.clone());
    health
        .dead_file_count
        .get_or_insert(dead.unused_files.len());
    health
        .dead_export_count
        .get_or_insert(dead.unused_exports.len());
    health.total_export_count.get_or_insert(dead.total_exports);
    health.circular_deps.get_or_insert(dead.cycles.len());
    health.entry_points = dead.entry_points.clone();
    health.cycle_paths = dead.cycles.iter().map(|c| c.path.clone()).collect();
    health.export_counts = dead.export_count_by_file.clone();
    if health.targets || (health.score && health.duplication_pct.is_none()) {
        let config = crate::config::load_config(&root)?.config;
        let dupes = crate::dupes::analyze_dupes(&root, &files, &config.dupes_options())?;
        health
            .duplication_pct
            .get_or_insert(dupes.stats.duplication_percentage);
        health.attach_evidence(&dead, &dupes);
    }
    let report = analyze_health(&root, &analyses, &graph, &health)?;
    Ok((Project { root, files, graph }, report))
}
