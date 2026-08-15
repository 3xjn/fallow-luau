//! Integration tests for dead-code, dupes, audit, explain, parity schema.

use std::path::PathBuf;

use fallow_luau::{
    analyze_audit, analyze_dead_code, analyze_dupes, build_require_graph, discover_files,
    explain_rule, schema_manifest, AuditOptions, DeadCodeOptions, DeadKind, DupesOptions,
};

fn fixtures(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn schema_lists_parity_commands() {
    let s = schema_manifest(true);
    assert!(s.get("_meta").is_some());
    assert_eq!(s["parity_doc"], "docs/parity.md");
    let names: Vec<_> = s["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for need in ["dead-code", "dupes", "audit", "explain", "mcp"] {
        assert!(names.contains(&need), "missing {need}");
    }
}

#[test]
fn dead_code_finds_unused_file_export_and_local() {
    let root = fixtures("dead_code");
    let files = discover_files(&root);
    let graph = build_require_graph(&root, &files).unwrap();
    let report = analyze_dead_code(
        &root,
        &files,
        &graph,
        &DeadCodeOptions {
            explain: true,
            extra_entries: vec!["main.luau".into()],
        },
    )
    .unwrap();
    assert!(
        report.unused_files.iter().any(|f| f.contains("orphan")),
        "expected orphan unused: {:?}",
        report.unused_files
    );
    assert!(
        report
            .unused_exports
            .iter()
            .any(|f| f.name == "drop_me" && f.kind == DeadKind::UnusedExport),
        "expected drop_me unused export: {:?}",
        report.unused_exports
    );
    assert!(
        report
            .unused_locals
            .iter()
            .any(|f| f.name == "never_read"),
        "expected never_read local: {:?}",
        report.unused_locals
    );
    assert!(report._meta.is_some());
}

#[test]
fn cycles_detected() {
    let root = fixtures("cycles");
    let files = discover_files(&root);
    let graph = build_require_graph(&root, &files).unwrap();
    let report = analyze_dead_code(&root, &files, &graph, &DeadCodeOptions::default()).unwrap();
    assert!(
        !report.cycles.is_empty(),
        "expected require cycle, got {:?}",
        report.cycles
    );
}

#[test]
fn dupes_finds_cloned_score_fn() {
    let root = fixtures("dupes");
    let files = discover_files(&root);
    let report = analyze_dupes(
        &root,
        &files,
        &DupesOptions {
            explain: true,
            min_tokens: 20,
            min_lines: 5,
            min_occurrences: 2,
        },
    )
    .unwrap();
    assert!(
        !report.clone_groups.is_empty(),
        "expected clone groups, stats={:?}",
        report.stats
    );
    assert!(report.stats.duplication_percentage > 0.0);
}

#[test]
fn explain_unused_file() {
    let v = explain_rule("unused-file");
    assert_eq!(v["id"], "unused-file");
    assert!(v.get("_meta").is_some());
}

#[test]
fn audit_runs_on_dead_code_fixture() {
    let root = fixtures("dead_code");
    let report = analyze_audit(
        &root,
        &AuditOptions {
            explain: true,
            ..AuditOptions::default()
        },
    )
    .unwrap();
    assert!(report._meta.is_some());
    // orphan file should fail the audit
    assert!(
        matches!(
            report.verdict,
            fallow_luau::AuditVerdict::Fail | fallow_luau::AuditVerdict::Warn
        ),
        "verdict={:?}",
        report.verdict
    );
}
