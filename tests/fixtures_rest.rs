//! Remaining parity surfaces: inspect, trace, flags, types, score, config, viz, suppressions.

use std::path::PathBuf;

use fallow_luau::{
    analyze_dead_code, analyze_flags, analyze_project, build_require_graph, collect_suppressions,
    discover_files, init_config, inspect_target, load_config, render_viz_html, schema_manifest,
    trace_symbol, DeadCodeOptions, DeadKind, HealthOptions, InitFormat, ProjectOptions,
};

fn fixtures(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn schema_marks_parity_complete() {
    let s = schema_manifest(false);
    assert_eq!(s["status"], "parity-complete");
    let names: Vec<_> = s["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for need in [
        "inspect", "trace", "watch", "init", "config", "suppressions", "report", "flags", "viz",
    ] {
        assert!(names.contains(&need), "missing {need} in {names:?}");
        let cmd = s["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == need)
            .unwrap();
        assert_eq!(cmd["status"], "done");
    }
}

#[test]
fn flags_detect_ff_and_settings() {
    let report = analyze_flags(&fixtures("flags"), true).unwrap();
    assert!(report.count >= 2, "flags={:?}", report.flags);
    assert!(report._meta.is_some());
}

#[test]
fn unused_type_detected() {
    let root = fixtures("types");
    let files = discover_files(&root);
    let graph = build_require_graph(&root, &files).unwrap();
    let report = analyze_dead_code(&root, &files, &graph, &DeadCodeOptions::default()).unwrap();
    assert!(
        report
            .unused_types
            .iter()
            .any(|f| f.name == "NeverUsed" && f.kind == DeadKind::UnusedType),
        "unused_types={:?}",
        report.unused_types
    );
}

#[test]
fn inspect_and_trace_require_graph() {
    let root = fixtures("require_graph");
    let inspect = inspect_target(&root, "main.luau", None, true).unwrap();
    assert_eq!(inspect.path, "main.luau");
    assert!(inspect.imports.iter().any(|i| i.contains("util")));
    assert!(inspect._meta.is_some());

    let trace = trace_symbol(&root, "util.luau", Some("add"), 2, true).unwrap();
    assert!(
        trace.caller_files.iter().any(|c| c.contains("main")),
        "callers={:?}",
        trace.caller_files
    );
}

#[test]
fn suppressions_inventory() {
    let report = collect_suppressions(&fixtures("suppress"), true).unwrap();
    assert!(report.totals.markers >= 1);
    assert!(report.suppressions[0].text.contains("fallow-luau-ignore"));
}

#[test]
fn init_and_config_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = init_config(dir.path(), InitFormat::Json).unwrap();
    assert!(path.exists());
    let resolved = load_config(dir.path()).unwrap();
    assert!(resolved.path.is_some());
    assert_eq!(resolved.config.health.max_cyclomatic, 20);
}

#[test]
fn health_score_present() {
    let (_p, report) = analyze_project(
        &fixtures("density"),
        &ProjectOptions {
            health: HealthOptions {
                score: true,
                hotspots: false,
                targets: false,
                explain: false,
                ..HealthOptions::default()
            },
        },
    )
    .unwrap();
    let hs = report.health_score.expect("health_score");
    assert!(hs.score <= 100.0);
    assert!(["A", "B", "C", "D", "F"].contains(&hs.grade.as_str()));
    assert_eq!(hs.formula_version, 2);
}

#[test]
fn viz_html_contains_nodes() {
    let html = render_viz_html(&fixtures("require_graph")).unwrap();
    assert!(html.contains("fallow-luau"));
    assert!(html.contains("treemap"));
    assert!(html.contains("main.luau") || html.contains("util.luau"));
}
