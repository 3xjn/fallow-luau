//! `audit` — combined dead-code + health + dupes, optionally scoped to changed files.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::dead_code::{analyze_dead_code, DeadCodeOptions, DeadCodeReport};
use crate::discover::discover_files;
use crate::dupes::{analyze_dupes, DupesOptions, DupesReport};
use crate::graph::build_require_graph;
use crate::health::{analyze_health, FileAnalysis, HealthOptions, HealthReport};
use crate::complexity::analyze_functions;
use crate::discover::is_test_path;
use crate::graph::display_rel;
use full_moon::parse;

#[derive(Debug, Clone, Default)]
pub struct AuditOptions {
    pub explain: bool,
    pub changed_since: Option<String>,
    pub health: HealthOptions,
    pub dead: DeadCodeOptions,
    pub dupes: DupesOptions,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    pub schema_version: u32,
    pub root: String,
    pub verdict: AuditVerdict,
    pub changed_files: Option<Vec<String>>,
    pub dead_code: DeadCodeReport,
    pub health: HealthReport,
    pub dupes: DupesReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditVerdict {
    Pass,
    Warn,
    Fail,
}

pub fn analyze_audit(root: &Path, opts: &AuditOptions) -> Result<AuditReport, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    let all_files = discover_files(&root);
    let changed = opts
        .changed_since
        .as_ref()
        .map(|rev| git_changed_files(&root, rev))
        .transpose()?;

    let files: Vec<PathBuf> = if let Some(ref ch) = changed {
        let set: BTreeSet<_> = ch.iter().cloned().collect();
        all_files
            .into_iter()
            .filter(|f| set.contains(&display_rel(&root, f)))
            .collect()
    } else {
        all_files
    };

    // Graph always built on full discovery for reachability accuracy when not changed-scoped;
    // when changed-scoped, build on the scoped set (best-effort gate).
    let graph_files = if changed.is_some() {
        // Include requires of changed files: use full discover for graph, filter findings later
        discover_files(&root)
    } else {
        files.clone()
    };
    let graph = build_require_graph(&root, &graph_files)?;

    let mut dead_opts = opts.dead.clone();
    dead_opts.explain = false;
    let mut dead = analyze_dead_code(&root, &graph_files, &graph, &dead_opts)?;
    if let Some(ref ch) = changed {
        let set: BTreeSet<_> = ch.iter().cloned().collect();
        dead.findings.retain(|f| set.contains(&f.path));
        dead.unused_files.retain(|f| set.contains(f));
        dead.unused_exports.retain(|f| set.contains(&f.path));
        dead.unused_locals.retain(|f| set.contains(&f.path));
    }

    let analysis_files = if changed.is_some() { &files } else { &graph_files };
    let mut analyses = Vec::new();
    for file in analysis_files {
        let source =
            std::fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
        let path = display_rel(&root, file);
        let ast = parse(&source).map_err(|e| format!("parse {path}: {e:?}"))?;
        analyses.push(FileAnalysis {
            is_test: is_test_path(Path::new(&path)),
            path,
            abs: file.clone(),
            lines: source.lines().count(),
            functions: analyze_functions(&ast),
        });
    }
    let mut health_opts = opts.health.clone();
    health_opts.explain = false;
    health_opts.dead_ratio_override = Some(dead.dead_ratio_by_file.clone());
    let health = analyze_health(&root, &analyses, &graph, &health_opts);

    let mut dupes_opts = opts.dupes.clone();
    dupes_opts.explain = false;
    let dupes = analyze_dupes(&root, analysis_files, &dupes_opts)?;

    let verdict = {
        let fail = !dead.unused_files.is_empty()
            || health
                .findings
                .as_ref()
                .map(|f| !f.is_empty())
                .unwrap_or(false);
        let warn = !dead.unused_exports.is_empty()
            || !dead.cycles.is_empty()
            || !dupes.clone_groups.is_empty();
        if fail {
            AuditVerdict::Fail
        } else if warn {
            AuditVerdict::Warn
        } else {
            AuditVerdict::Pass
        }
    };

    Ok(AuditReport {
        schema_version: 1,
        root: root.display().to_string(),
        verdict,
        changed_files: changed,
        dead_code: dead,
        health,
        dupes,
        _meta: if opts.explain {
            Some(serde_json::json!({
                "docs": "https://docs.fallow.tools/cli/audit",
                "parity": "docs/parity.md",
                "verdict": "fail on unused files or complexity findings; warn on unused exports, cycles, or clones"
            }))
        } else {
            None
        },
    })
}

fn git_changed_files(root: &Path, rev: &str) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "diff",
            "--name-only",
            "--diff-filter=ACMR",
            rev,
            "--",
            "*.lua",
            "*.luau",
        ])
        .output()
        .map_err(|e| format!("git diff: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().replace('\\', "/"))
        .filter(|l| !l.is_empty())
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}
