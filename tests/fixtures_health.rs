//! Fixture-backed integration tests for parse/graph/health.

use std::collections::BTreeMap;
use std::path::PathBuf;

use fallow_luau::{
    analyze_functions, analyze_project, build_require_graph, discover_files, list_report,
    schema_manifest, HealthOptions, ProjectOptions, RequireKind,
};
use full_moon::parse;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn schema_lists_core_commands() {
    let schema = schema_manifest(true);
    assert_eq!(schema["name"], "fallow-luau");
    assert!(schema.get("_meta").is_some());
    let commands = schema["commands"].as_array().unwrap();
    let names: Vec<_> = commands
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"schema"));
    assert!(names.contains(&"list"));
    assert!(names.contains(&"health"));
}

#[test]
fn require_graph_resolves_string_literals() {
    let root = fixtures_root().join("require_graph");
    let files = discover_files(&root);
    let graph = build_require_graph(&root, &files).unwrap();
    assert!(graph.files.iter().any(|f| f.ends_with("main.luau")));
    let resolved = graph
        .edges
        .iter()
        .find(|e| e.from.ends_with("main.luau") && e.to.as_deref().is_some_and(|t| t.contains("util")))
        .expect("main -> util edge");
    assert_eq!(resolved.kind, RequireKind::StringLiteral);
}

#[test]
fn dynamic_require_is_unresolved_edge() {
    let root = fixtures_root().join("require_graph");
    let files = discover_files(&root);
    let graph = build_require_graph(&root, &files).unwrap();
    let dyn_edge = graph
        .unresolved
        .iter()
        .find(|e| e.from.contains("dynamic") && e.kind == RequireKind::Dynamic)
        .expect("dynamic require");
    assert!(dyn_edge.to.is_none());
    assert!(graph
        .unresolved
        .iter()
        .any(|e| e.kind == RequireKind::Loadstring));
}

#[test]
fn list_report_includes_meta_when_explain() {
    let root = fixtures_root().join("require_graph");
    let files = discover_files(&root);
    let graph = build_require_graph(&root, &files).unwrap();
    let report = list_report(&root.display().to_string(), &graph, true);
    assert!(report.get("_meta").is_some());
    assert!(report["file_count"].as_u64().unwrap() >= 2);
}

#[test]
fn nested_inner_function_scored_despite_large_file() {
    let path = fixtures_root().join("nested_inner/big_file.luau");
    let src = std::fs::read_to_string(&path).unwrap();
    let file_lines = src.lines().count();
    assert!(
        file_lines > 100,
        "fixture file should be large, got {file_lines}"
    );

    let ast = parse(&src).unwrap();
    let fns = analyze_functions(&ast);
    let inner = fns
        .iter()
        .find(|f| f.name.ends_with("inner") || f.name.contains("/inner"))
        .unwrap_or_else(|| panic!("expected nested inner unit, got {fns:?}"));

    // ~40-line inner function must be its own unit with real complexity,
    // even though the enclosing file is much larger.
    assert!(
        (38..=45).contains(&inner.lines),
        "inner lines={} (expected ~40); file_lines={file_lines}; fns={fns:?}",
        inner.lines
    );
    assert!(
        inner.cyclomatic >= 8,
        "inner cyclomatic too low: {}",
        inner.cyclomatic
    );
    assert!(
        inner.cognitive >= 8,
        "inner cognitive too low: {}",
        inner.cognitive
    );
    assert!(
        matches!(
            inner.unit_size_bin,
            fallow_luau::RiskBin::HighRisk | fallow_luau::RiskBin::VeryHighRisk
        ),
        "inner should land in high SIG size bin"
    );

    // File-level analysis still sees the nested unit.
    let (project, report) = analyze_project(
        &fixtures_root().join("nested_inner"),
        &ProjectOptions {
            health: HealthOptions {
                hotspots: false,
                targets: false,
                explain: true,
                ..HealthOptions::default()
            },
        },
    )
    .unwrap();
    assert_eq!(project.files.len(), 1);
    let scores = report.file_scores.as_ref().unwrap();
    assert_eq!(scores[0].function_count, fns.len());
    assert!(report._meta.is_some());
    let large = report.large_functions.as_ref();
    // 40-line function is under default 60 threshold — still present as a unit
    // in findings/file scores even when not in large_functions.
    let _ = large;
}

#[test]
fn density_and_mi_on_dense_fixture() {
    let (_project, report) = analyze_project(
        &fixtures_root().join("density"),
        &ProjectOptions {
            health: HealthOptions {
                hotspots: false,
                targets: true,
                explain: false,
                max_cyclomatic: 20,
                max_cognitive: 15,
                ..HealthOptions::default()
            },
        },
    )
    .unwrap();
    let score = &report.file_scores.as_ref().unwrap()[0];
    assert!(
        score.complexity_density > 0.3,
        "density={}",
        score.complexity_density
    );
    // MI = 100 - density*30 - 0 - fan_out_penalty(0)
    let expected = 100.0 - score.complexity_density * 30.0;
    assert!(
        (score.maintainability_index - expected).abs() < 0.05,
        "mi={} expected~{}",
        score.maintainability_index,
        expected
    );
}

#[test]
fn sig_unit_size_profile_present() {
    let (_project, report) = analyze_project(
        &fixtures_root().join("nested_inner"),
        &ProjectOptions::default(),
    )
    .unwrap();
    let profile = &report.vital_signs.unit_size_profile;
    let sum = profile.low_risk + profile.medium_risk + profile.high_risk + profile.very_high_risk;
    assert!(
        (sum - 100.0).abs() < 0.01 || sum == 0.0,
        "profile percentages should sum to 100, got {sum}"
    );
}

#[test]
fn hotspots_use_normalized_churn_and_density() {
    let now = 1_700_000_000_i64;
    let mut churn = BTreeMap::new();
    // Recent commits on the dense file.
    churn.insert(
        "dense.luau".into(),
        vec![now - 86400, now - 2 * 86400, now - 3 * 86400],
    );

    let (_project, report) = analyze_project(
        &fixtures_root().join("density"),
        &ProjectOptions {
            health: HealthOptions {
                hotspots: true,
                targets: true,
                file_scores: true,
                complexity: true,
                churn_override: Some(churn),
                now_unix: Some(now),
                min_commits: 1,
                since_days: 180.0,
                ..HealthOptions::default()
            },
        },
    )
    .unwrap();
    let hotspots = report.hotspots.as_ref().unwrap();
    assert!(!hotspots.is_empty());
    assert!(
        hotspots[0].score > 0.0,
        "expected positive hotspot score, got {}",
        hotspots[0].score
    );
    // Single file → normalized factors are 1 → score ≈ 100.
    assert!(
        hotspots[0].score > 90.0,
        "single-file hotspot should be near 100, got {}",
        hotspots[0].score
    );
}

#[test]
fn static_estimated_coverage_from_test_require() {
    let (_project, report) = analyze_project(
        &fixtures_root().join("coverage_graph"),
        &ProjectOptions {
            health: HealthOptions {
                hotspots: false,
                targets: false,
                ..HealthOptions::default()
            },
        },
    )
    .unwrap();
    let math = report
        .file_scores
        .as_ref()
        .unwrap()
        .iter()
        .find(|s| s.path.contains("mathlib.luau") && !s.path.contains("spec"))
        .expect("mathlib file score");
    // Directly required by test → cov 85% → CRAP lower than fully untested.
    // CC for add is 1+2=3 (if + elseif); crap(3, 85) = 9*(0.15)^3 + 3 ≈ 3.03
    assert!(
        math.crap_max < 10.0,
        "expected low CRAP with 85% estimated coverage, got {}",
        math.crap_max
    );
}
