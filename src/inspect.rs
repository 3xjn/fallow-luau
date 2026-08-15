//! `inspect` — compose evidence for one file or returned key.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::complexity::analyze_functions;
use crate::dead_code::{analyze_dead_code, DeadCodeOptions};
use crate::discover::discover_files;
use crate::dupes::{analyze_dupes, DupesOptions};
use crate::graph::{build_require_graph, display_rel, fan_counts};
use crate::health::{analyze_health, FileAnalysis, HealthOptions};
use crate::discover::is_test_path;
use full_moon::parse;

#[derive(Debug, Clone, Serialize)]
pub struct InspectReport {
    pub schema_version: u32,
    pub root: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub fan_in: usize,
    pub fan_out: usize,
    pub functions: serde_json::Value,
    pub dead_findings: serde_json::Value,
    pub clone_groups: serde_json::Value,
    pub file_score: serde_json::Value,
    pub importers: Vec<String>,
    pub imports: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

pub fn inspect_target(
    root: &Path,
    target: &str,
    symbol: Option<&str>,
    explain: bool,
) -> Result<InspectReport, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    let files = discover_files(&root);
    let graph = build_require_graph(&root, &files)?;
    let rel = normalize_target(&root, target, &graph.files)?;

    let abs = root.join(&rel);
    let source = std::fs::read_to_string(&abs).map_err(|e| format!("read {rel}: {e}"))?;
    let ast = parse(&source).map_err(|e| format!("parse {rel}: {e:?}"))?;
    let functions = analyze_functions(&ast);
    let functions = if let Some(sym) = symbol {
        functions
            .into_iter()
            .filter(|f| f.name == sym || f.name.ends_with(&format!("/{sym}")))
            .collect::<Vec<_>>()
    } else {
        functions
    };

    let dead = analyze_dead_code(
        &root,
        &files,
        &graph,
        &DeadCodeOptions {
            explain: false,
            ..DeadCodeOptions::default()
        },
    )?;
    let dead_findings: Vec<_> = dead
        .findings
        .into_iter()
        .filter(|f| {
            f.path == rel
                && symbol
                    .map(|s| f.name == s || f.name.contains(s))
                    .unwrap_or(true)
        })
        .collect();

    let dupes = analyze_dupes(
        &root,
        &files,
        &DupesOptions {
            explain: false,
            min_tokens: 20,
            min_lines: 4,
            min_occurrences: 2,
        },
    )?;
    let clone_groups: Vec<_> = dupes
        .clone_groups
        .into_iter()
        .filter(|g| g.instances.iter().any(|i| i.path == rel))
        .collect();

    let mut analyses = Vec::new();
    for file in &files {
        let p = display_rel(&root, file);
        if p != rel {
            continue;
        }
        let src = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let a = parse(&src).map_err(|e| format!("{e:?}"))?;
        analyses.push(FileAnalysis {
            is_test: is_test_path(Path::new(&p)),
            path: p,
            abs: file.clone(),
            lines: src.lines().count(),
            functions: analyze_functions(&a),
        });
    }
    let mut hopts = HealthOptions {
        explain: false,
        hotspots: false,
        targets: false,
        dead_ratio_override: Some(dead.dead_ratio_by_file.clone()),
        ..HealthOptions::default()
    };
    hopts.complexity = true;
    hopts.file_scores = true;
    let health = analyze_health(&root, &analyses, &graph, &hopts);
    let file_score = health
        .file_scores
        .and_then(|s| s.into_iter().find(|f| f.path == rel));

    let fans = fan_counts(&graph);
    let (fan_in, fan_out) = fans.get(&rel).copied().unwrap_or((0, 0));
    let mut importers = Vec::new();
    let mut imports = Vec::new();
    for e in &graph.edges {
        if e.to.as_deref() == Some(rel.as_str()) {
            importers.push(e.from.clone());
        }
        if e.from == rel {
            if let Some(to) = &e.to {
                imports.push(to.clone());
            }
        }
    }
    importers.sort();
    importers.dedup();
    imports.sort();
    imports.dedup();

    Ok(InspectReport {
        schema_version: 1,
        root: root.display().to_string(),
        path: rel,
        symbol: symbol.map(|s| s.to_string()),
        fan_in,
        fan_out,
        functions: serde_json::to_value(functions).unwrap(),
        dead_findings: serde_json::to_value(dead_findings).unwrap(),
        clone_groups: serde_json::to_value(clone_groups).unwrap(),
        file_score: serde_json::to_value(file_score).unwrap(),
        importers,
        imports,
        _meta: if explain {
            Some(serde_json::json!({
                "docs": "https://docs.fallow.tools/cli/inspect",
                "note": "Composes graph, complexity, dead-code, and dupes evidence for one path/key."
            }))
        } else {
            None
        },
    })
}

fn normalize_target(root: &Path, target: &str, known: &[String]) -> Result<String, String> {
    let t = target.replace('\\', "/");
    if known.iter().any(|k| k == &t) {
        return Ok(t);
    }
    let abs = if Path::new(target).is_absolute() {
        PathBuf::from(target)
    } else {
        root.join(target)
    };
    if abs.exists() {
        return Ok(display_rel(root, &abs));
    }
    // fuzzy: suffix match
    if let Some(hit) = known.iter().find(|k| k.ends_with(&t) || k.ends_with(target)) {
        return Ok(hit.clone());
    }
    Err(format!("unknown file `{target}`"))
}
