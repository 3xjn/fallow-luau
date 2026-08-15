use std::path::{Path, PathBuf};

use full_moon::parse;
use serde::Serialize;

use crate::complexity::analyze_functions;
use crate::discover::{discover_files, is_test_path};
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

pub fn analyze_project(root: &Path, opts: &ProjectOptions) -> Result<(Project, HealthReport), String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    let files = discover_files(&root);
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

    let report = analyze_health(&root, &analyses, &graph, &opts.health);
    Ok((
        Project {
            root,
            files,
            graph,
        },
        report,
    ))
}
