//! `audit` — combined dead-code + health + dupes, optionally scoped to changed files.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::complexity::analyze_functions;
use crate::dead_code::{analyze_dead_code, DeadCodeOptions, DeadCodeReport};
use crate::discover::discover_configured_files;
use crate::discover::is_test_path;
use crate::dupes::{analyze_dupes, DupesOptions, DupesReport};
use crate::graph::build_require_graph;
use crate::graph::display_rel;
use crate::health::{analyze_health, FileAnalysis, HealthOptions, HealthReport};
use full_moon::parse;

#[derive(Debug, Clone, Default)]
pub struct AuditOptions {
    pub explain: bool,
    pub changed_since: Option<String>,
    pub gate: AuditGate,
    pub health: HealthOptions,
    pub dead: DeadCodeOptions,
    pub dupes: DupesOptions,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditGate {
    #[default]
    NewOnly,
    All,
}

#[derive(Debug, Clone, Serialize)]
pub struct Attribution {
    pub kind: String,
    pub path: String,
    pub name: String,
    pub introduced: bool,
}

impl AuditOptions {
    pub fn configured(root: &Path) -> Result<Self, String> {
        let config = crate::config::load_config(root)?.config;
        Ok(Self {
            health: config.health_options(),
            dupes: config.dupes_options(),
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    pub schema_version: u32,
    pub root: String,
    pub verdict: AuditVerdict,
    pub gate: AuditGate,
    pub attribution: Vec<Attribution>,
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
    let all_files = discover_configured_files(&root)?;
    let changed = opts
        .changed_since
        .as_ref()
        .map(|rev| git_changed_files(&root, rev))
        .transpose()?;

    // Analyze the full graph and clone corpus, then scope reported findings.
    // Otherwise a changed file copied from an unchanged file has no sibling.
    let graph_files = all_files;
    let graph = build_require_graph(&root, &graph_files)?;

    let mut dead_opts = opts.dead.clone();
    dead_opts.explain = false;
    let mut dead = analyze_dead_code(&root, &graph_files, &graph, &dead_opts)?;

    let analysis_files = &graph_files;
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

    let mut dupes_opts = opts.dupes.clone();
    dupes_opts.explain = false;
    let mut dupes = analyze_dupes(&root, analysis_files, &dupes_opts)?;
    health_opts.dead_file_count = Some(dead.unused_files.len());
    health_opts.dead_export_count = Some(dead.unused_exports.len());
    health_opts.total_export_count = Some(dead.total_exports);
    health_opts.circular_deps = Some(dead.cycles.len());
    health_opts.duplication_pct = Some(dupes.stats.duplication_percentage);
    health_opts.entry_points = dead.entry_points.clone();
    health_opts.cycle_paths = dead.cycles.iter().map(|c| c.path.clone()).collect();
    health_opts.export_counts = dead.export_count_by_file.clone();
    health_opts.attach_evidence(&dead, &dupes);
    let mut health = analyze_health(&root, &analyses, &graph, &health_opts)?;
    if let Some(ch) = &changed {
        let set: BTreeSet<_> = ch.iter().cloned().collect();
        dead.cycles
            .retain(|c| c.path.iter().any(|p| set.contains(p)));
        dead.findings.retain(|f| {
            set.contains(&f.path)
                || (f.kind == crate::dead_code::DeadKind::CircularDependency
                    && dead.cycles.iter().any(|c| c.path.join(" → ") == f.name))
        });
        dead.unused_files.retain(|f| set.contains(f));
        dead.unused_exports.retain(|f| set.contains(&f.path));
        dead.unused_locals.retain(|f| set.contains(&f.path));
        dead.unused_types.retain(|f| set.contains(&f.path));
        dead.boundary_violations.retain(|f| set.contains(&f.path));
        dupes.retain_groups(|g| g.instances.iter().any(|i| set.contains(&i.path)));
        if let Some(items) = &mut health.findings {
            items.retain(|f| ch.contains(&f.path));
        }
        if let Some(items) = &mut health.large_functions {
            items.retain(|f| ch.contains(&f.path));
        }
        if let Some(items) = &mut health.file_scores {
            items.retain(|f| ch.contains(&f.path));
        }
        if let Some(items) = &mut health.targets {
            items.retain(|f| ch.contains(&f.path));
        }
        if let Some(items) = &mut health.hotspots {
            items.retain(|f| ch.contains(&f.path));
        }
    }

    let gate = if changed.is_some() {
        opts.gate
    } else {
        AuditGate::All
    };
    let current = issue_keys(&dead, &health, &dupes);
    let mut inherited = std::collections::BTreeMap::new();
    if gate == AuditGate::NewOnly {
        let snapshot = base_snapshot(&root, opts.changed_since.as_deref().unwrap())?;
        let base = analyze_audit(
            snapshot.path(),
            &AuditOptions {
                gate: AuditGate::All,
                ..AuditOptions::configured(snapshot.path())?
            },
        )?;
        for (key, _) in issue_keys(&base.dead_code, &base.health, &base.dupes) {
            *inherited.entry(key).or_insert(0usize) += 1;
        }
    }
    let mut verdict = AuditVerdict::Pass;
    let mut attribution = Vec::new();
    for ((kind, path, name), fail) in current {
        let count = inherited
            .entry((kind.clone(), path.clone(), name.clone()))
            .or_default();
        let introduced = *count == 0;
        *count = count.saturating_sub(1);
        if gate == AuditGate::All || introduced {
            if fail {
                verdict = AuditVerdict::Fail;
            } else if verdict != AuditVerdict::Fail {
                verdict = AuditVerdict::Warn;
            }
        }
        if gate == AuditGate::NewOnly {
            attribution.push(Attribution {
                kind,
                path,
                name,
                introduced,
            });
        }
    }

    Ok(AuditReport {
        schema_version: 1,
        root: root.display().to_string(),
        verdict,
        gate,
        attribution,
        changed_files: changed,
        dead_code: dead,
        health,
        dupes,
        _meta: if opts.explain {
            Some(serde_json::json!({
                "docs": "https://docs.fallow.tools/cli/audit",
                "parity": "docs/parity.md",
                "verdict": "fail on unused files, boundary violations, complexity or CRAP findings; warn on unused exports/locals/types, large functions, cycles, or clones",
                "scope": "Changed-file findings use full-project analysis, scores and vital signs. New-only compares structural issue keys against the specified ref, ignoring source-line movement. All skips base comparison. Without changed-since, all findings gate."
            }))
        } else {
            None
        },
    })
}

type IssueKey = (String, String, String);

fn issue_keys(
    dead: &DeadCodeReport,
    health: &HealthReport,
    dupes: &DupesReport,
) -> Vec<(IssueKey, bool)> {
    let mut result = Vec::new();
    for f in &dead.findings {
        let kind = serde_json::to_value(f.kind)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let fail = matches!(
            f.kind,
            crate::dead_code::DeadKind::UnusedFile | crate::dead_code::DeadKind::BoundaryViolation
        );
        result.push(((kind, f.path.clone(), f.name.clone()), fail));
    }
    for f in health.findings.iter().flatten() {
        result.push(((f.rule.clone(), f.path.clone(), f.name.clone()), true));
    }
    for f in health.large_functions.iter().flatten() {
        result.push((
            ("large-function".into(), f.path.clone(), f.name.clone()),
            false,
        ));
    }
    for g in &dupes.clone_groups {
        let mut paths: Vec<_> = g.instances.iter().map(|i| i.path.as_str()).collect();
        paths.sort();
        result.push((
            (
                "duplicates".into(),
                serde_json::to_string(&paths).unwrap(),
                g.fingerprint.clone(),
            ),
            false,
        ));
    }
    result
}

/// Read tracked source/config blobs without changing the checkout or registering a worktree.
fn base_snapshot(root: &Path, rev: &str) -> Result<tempfile::TempDir, String> {
    let git = |args: &[&str]| -> Result<Vec<u8>, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|e| format!("git snapshot: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "git snapshot: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(output.stdout)
    };
    // Resolve first so caller-controlled refs cannot be interpreted as options or paths.
    let commit = String::from_utf8(git(&[
        "rev-parse",
        "--verify",
        "--end-of-options",
        &format!("{rev}^{{commit}}"),
    ])?)
    .map_err(|e| e.to_string())?;
    let prefix =
        String::from_utf8(git(&["rev-parse", "--show-prefix"])?).map_err(|e| e.to_string())?;
    let listing = git(&["ls-tree", "-rz", "--full-tree", commit.trim()])?;
    let dir = tempfile::tempdir().map_err(|e| format!("create base snapshot: {e}"))?;
    for record in listing.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let record = std::str::from_utf8(record).map_err(|e| format!("non-UTF8 git path: {e}"))?;
        let (header, path) = record.split_once('\t').ok_or("invalid git tree record")?;
        let Some(path) = path.strip_prefix(prefix.trim_end_matches(['\r', '\n'])) else {
            continue;
        };
        let relative = Path::new(path);
        let config = matches!(
            path,
            ".fallow-luau.json" | "fallow-luau.toml" | ".fallow-luau.toml" | "default.project.json"
        );
        if !config && !crate::discover::is_luau_source(relative) {
            continue;
        }
        if relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(format!("unsupported snapshot path: {path}"));
        }
        let fields: Vec<_> = header.split_whitespace().collect();
        if fields.len() != 3 || fields[1] != "blob" || !matches!(fields[0], "100644" | "100755") {
            return Err(format!("unsupported source mode in base snapshot: {path}"));
        }
        let bytes = git(&["cat-file", "blob", fields[2]])?;
        let output = dir.path().join(relative);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("snapshot directory: {e}"))?;
        }
        std::fs::write(&output, bytes)
            .map_err(|e| format!("snapshot {}: {e}", output.display()))?;
    }
    Ok(dir)
}

fn git_changed_files(root: &Path, rev: &str) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "diff",
            "--name-only",
            "--relative",
            "-z",
            "--diff-filter=ACMR",
            "--end-of-options",
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
        .split('\0')
        .map(|l| l.replace('\\', "/"))
        .filter(|l| !l.is_empty())
        .collect();
    let untracked = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "*.lua",
            "*.luau",
        ])
        .output()
        .map_err(|e| format!("git ls-files: {e}"))?;
    if !untracked.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&untracked.stderr)
        ));
    }
    files.extend(
        String::from_utf8_lossy(&untracked.stdout)
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_owned),
    );
    files.sort();
    files.dedup();
    Ok(files)
}
