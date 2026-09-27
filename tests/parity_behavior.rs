use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/parity")
}
fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fallow-luau"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn report(root: &Path, args: &[&str]) -> Value {
    let out = cli(root, args);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
fn copy_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for entry in std::fs::read_dir(fixture()).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), dir.path().join(entry.file_name())).unwrap();
    }
    dir
}
fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn keyed_trace_reports_observed_method_bracket_and_direct_require_references() {
    let root = fixture();
    let keep = report(
        &root,
        &["trace", "module.luau", "--key", "keep", "--depth", "2"],
    );
    let edges = keep["callers"].as_array().unwrap();
    assert!(edges
        .iter()
        .any(|e| e["from"] == "uses.luau" && e["key"] == "keep" && e["line"] == 2));
    assert!(edges
        .iter()
        .any(|e| e["from"] == "uses.luau" && e["line"] == 4));
    assert!(edges
        .iter()
        .any(|e| e["from"] == "main.luau" && e.get("key").is_none()));
    assert!(!edges.iter().any(|e| e["from"] == "shadows.luau"));
    let constant = report(&root, &["trace", "module.luau", "--key", "constant"]);
    assert!(constant["callers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["line"] == 3));
    let drop = report(&root, &["trace", "module.luau", "--key", "drop"]);
    assert_eq!(drop["callers"], json!([]));
    assert!(!cli(&root, &["trace", "module.luau", "--key", "typo"])
        .status
        .success());
    let zero = report(&root, &["trace", "module.luau", "--depth", "0"]);
    assert_eq!(zero["callers"], json!([]));
    let dead = report(&root, &["dead-code"]);
    let unused: Vec<_> = dead["unused_exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(unused, ["drop"]);
}

#[test]
fn inspect_accepts_methods_and_exported_constants_and_rejects_ignored_files() {
    let dir = copy_fixture();
    let method = report(
        dir.path(),
        &["inspect", "module.luau", "--symbol", "M.keep"],
    );
    assert_eq!(method["functions"].as_array().unwrap().len(), 1);
    report(
        dir.path(),
        &["inspect", "module.luau", "--symbol", "constant"],
    );
    std::fs::write(
        dir.path().join(".fallow-luau.json"),
        r#"{"ignore":["shadows.luau"]}"#,
    )
    .unwrap();
    let ignored = cli(dir.path(), &["trace", "shadows.luau"]);
    assert!(!ignored.status.success());
    assert!(String::from_utf8_lossy(&ignored.stderr).contains("outside analyzed scope"));
}

#[test]
fn crap_findings_and_coverage_targets_include_actionable_evidence() {
    let health = report(&fixture(), &["health"]);
    assert_eq!(health["coverage_model"], "static_estimated");
    let risky: Vec<_> = health["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["path"] == "risky.luau")
        .collect();
    assert_eq!(risky.len(), 2);
    assert!(risky
        .iter()
        .all(|f| f["crap"] == 30.0 && f["rule"] == "fallow-luau/high-crap"));
    let target = health["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["path"] == "risky.luau")
        .unwrap();
    assert_eq!(target["category"], "add_test_coverage");
    assert_eq!(target["effort"], "low");
    assert_eq!(target["efficiency"], target["priority"]);
    assert_eq!(target["evidence"]["functions"][0]["line"], 1);
    let score = health["file_scores"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "risky.luau")
        .unwrap();
    // 10 CC / 15 lines, dampening 15/50 => density penalty 6.
    assert_eq!(score["maintainability_index"], 94.0);
    let density = score["complexity_density"].as_f64().unwrap();
    assert!((target["priority"].as_f64().unwrap() - (density * 30.0 + 3.0)).abs() < 0.02);
}

#[test]
fn suppressions_affect_findings_but_preserve_raw_metrics_and_ignore_strings() {
    let dir = copy_fixture();
    let root = dir.path();
    let before = report(root, &["health"]);
    let source = std::fs::read_to_string(root.join("risky.luau")).unwrap();
    std::fs::write(
        root.join("risky.luau"),
        format!("-- fallow-ignore-file complexity, coverage-gaps -- tested elsewhere\n{source}"),
    )
    .unwrap();
    std::fs::write(
        root.join("fake.luau"),
        "return [[-- fallow-ignore-file unused-file]]",
    )
    .unwrap();
    let after = report(root, &["health"]);
    assert!(!after["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == "risky.luau"));
    let score = |v: &Value| {
        v["file_scores"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == "risky.luau")
            .unwrap()["total_cyclomatic"]
            .clone()
    };
    assert_eq!(score(&before), score(&after));
    let inventory = report(root, &["suppressions"]);
    assert_eq!(inventory["totals"]["markers"], 1);
    assert_eq!(inventory["suppressions"][0]["reason"], "tested elsewhere");
    let dead = report(root, &["dead-code"]);
    assert!(dead["unused_files"]
        .as_array()
        .unwrap()
        .contains(&json!("fake.luau")));
    std::fs::write(
        root.join("locals.luau"),
        "-- fallow-ignore-next-line unused-local\nlocal hidden = 1\nlocal visible = 2\nreturn {}\n",
    )
    .unwrap();
    let dead = report(root, &["dead-code"]);
    let names: Vec<_> = dead["unused_locals"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["path"] == "locals.luau")
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["visible"]);
}

#[test]
fn boundaries_enforce_direction_and_report_configuration_errors() {
    let dir = copy_fixture();
    let config = json!({"boundaries":[
        {"name":"clients", "paths":["uses.luau", "shadows.luau"], "allow_imports_from":[]},
        {"name":"core", "paths":["module.luau"], "allow_imports_from":["clients"]}
    ]});
    let path = dir.path().join(".fallow-luau.json");
    std::fs::write(&path, config.to_string()).unwrap();
    let dead = report(dir.path(), &["dead-code"]);
    assert_eq!(dead["boundary_violations"].as_array().unwrap().len(), 3);
    assert!(dead["boundary_violations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == "uses.luau" && f["line"] == 1));
    assert!(!cli(dir.path(), &["audit"]).status.success());
    let mut allowed = config.clone();
    allowed["boundaries"][0]["allow_imports_from"] = json!(["core"]);
    std::fs::write(&path, allowed.to_string()).unwrap();
    assert_eq!(
        report(dir.path(), &["dead-code"])["boundary_violations"],
        json!([])
    );
    allowed["boundaries"][0]["allow_imports_from"] = json!(["typo"]);
    std::fs::write(&path, allowed.to_string()).unwrap();
    assert!(!cli(dir.path(), &["dead-code"]).status.success());
}

#[test]
fn baseline_roundtrip_and_errors_are_observable() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = dir.path().join("baseline.json");
    let path = baseline.to_str().unwrap();
    let before = report(&fixture(), &["health", "--save-baseline", path]);
    assert!(!before["targets"].as_array().unwrap().is_empty());
    let after = report(&fixture(), &["health", "--baseline", path]);
    assert_eq!(after["targets"], json!([]));
    for invalid in [
        "not json",
        r#"{"schema_version":1}"#,
        r#"{"schema_version":2,"keys":[]}"#,
    ] {
        std::fs::write(&baseline, invalid).unwrap();
        assert!(!cli(&fixture(), &["health", "--baseline", path])
            .status
            .success());
    }
    assert!(!cli(
        &fixture(),
        &["health", "--save-baseline", dir.path().to_str().unwrap()]
    )
    .status
    .success());
    assert!(!cli(
        &fixture(),
        &[
            "health",
            "--baseline",
            dir.path().join("missing").to_str().unwrap()
        ]
    )
    .status
    .success());
}

#[test]
fn changed_audit_compares_clones_with_unchanged_files_and_includes_untracked() {
    let dir = copy_fixture();
    let root = dir.path();
    git(root, &["init"]);
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    );
    std::fs::copy(root.join("risky.luau"), root.join("new copy.luau")).unwrap();
    let out = cli(root, &["audit", "--changed-since", "HEAD"]);
    let audit: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(audit["changed_files"], json!(["new copy.luau"]));
    let groups = audit["dupes"]["clone_groups"].as_array().unwrap();
    assert!(groups.iter().any(|g| {
        let paths: Vec<_> = g["instances"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["path"].as_str().unwrap())
            .collect();
        paths.contains(&"risky.luau") && paths.contains(&"new copy.luau")
    }));
    assert!(audit["health"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|f| f["path"] == "new copy.luau"));
    assert_eq!(
        audit["health"]["health_score"],
        report(root, &["health"])["health_score"]
    );
    git(root, &["add", "new copy.luau"]);
    let tracked: Value =
        serde_json::from_slice(&cli(root, &["audit", "--changed-since", "HEAD"]).stdout).unwrap();
    assert_eq!(tracked["dupes"], audit["dupes"]);
}

#[test]
fn new_only_gate_preserves_inherited_findings_without_failing_comment_edits() {
    let dir = copy_fixture();
    let root = dir.path();
    git(root, &["init"]);
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    );
    let risky = root.join("risky.luau");
    let source = std::fs::read_to_string(&risky).unwrap();
    std::fs::write(
        &risky,
        format!("-- harmless comment shifts finding lines\n{source}"),
    )
    .unwrap();
    let audit = report(root, &["audit", "--changed-since", "HEAD"]);
    assert_eq!(audit["verdict"], "pass");
    let attribution = audit["attribution"].as_array().unwrap();
    assert!(!attribution.is_empty());
    assert!(attribution.iter().all(|a| a["introduced"] == false));
    assert!(!audit["health"]["findings"].as_array().unwrap().is_empty());
    assert!(
        !cli(root, &["audit", "--changed-since", "HEAD", "--gate", "all"])
            .status
            .success()
    );
    // A new unused file must still fail even with an existing baseline.
    std::fs::write(root.join("orphan.luau"), "return {}").unwrap();
    let out = cli(root, &["audit", "--changed-since", "HEAD"]);
    assert!(!out.status.success());
    let audit: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(audit["attribution"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["kind"] == "unused_file" && a["introduced"] == true));
    assert!(!cli(root, &["audit", "--changed-since", "not-a-ref"])
        .status
        .success());
}

#[test]
fn duplicate_tokenizer_ignores_long_comments_and_handles_long_strings() {
    let dir = tempfile::tempdir().unwrap();
    let source = include_str!("fixtures/parity/risky.luau");
    std::fs::write(
        dir.path().join("a.luau"),
        format!("--[=[\n{source}\n]=]\nreturn 1"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.luau"),
        format!("--[=[\n{source}\n]=]\nreturn 2"),
    )
    .unwrap();
    assert_eq!(report(dir.path(), &["dupes"])["clone_groups"], json!([]));
    std::fs::write(
        dir.path().join("a.luau"),
        format!("local text = [=[first\nvalue]=]\n{source}"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.luau"),
        format!("local text = [=[second\nvalue]=]\n{source}"),
    )
    .unwrap();
    assert!(!report(dir.path(), &["dupes"])["clone_groups"]
        .as_array()
        .unwrap()
        .is_empty());
    let b = std::fs::read_to_string(dir.path().join("b.luau")).unwrap();
    std::fs::write(
        dir.path().join("b.luau"),
        format!("-- fallow-ignore-file code-duplication\n{b}"),
    )
    .unwrap();
    let suppressed = report(dir.path(), &["dupes"]);
    assert!(suppressed["clone_groups"]
        .as_array()
        .unwrap()
        .iter()
        .all(|g| g["instances"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["path"] != "b.luau")));
    assert!(!cli(dir.path(), &["dupes", "--min-tokens", "0"])
        .status
        .success());
}

#[test]
fn cycle_evidence_only_follows_real_edges() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.luau"),
        "require('./b')\nrequire('./c')\nreturn function() end",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.luau"),
        "require('./main')\nreturn function() end",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("c.luau"),
        "require('./main')\nreturn function() end",
    )
    .unwrap();
    let graph = report(dir.path(), &["list"]);
    let dead = report(dir.path(), &["dead-code"]);
    assert_eq!(dead["cycles"].as_array().unwrap().len(), 1);
    let path = dead["cycles"][0]["path"].as_array().unwrap();
    assert_eq!(path.first(), path.last());
    for pair in path.windows(2) {
        assert!(
            graph["edges"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["from"] == pair[0] && e["to"] == pair[1]),
            "{pair:?}"
        );
    }
    let health = report(dir.path(), &["health"]);
    assert_eq!(health["targets"].as_array().unwrap().len(), 3);
    assert!(health["targets"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["category"] == "circular_dep"
            && t["evidence"]["cycle_path"] == dead["cycles"][0]["path"]));
}

#[test]
fn target_order_uses_efficiency_rather_than_raw_priority() {
    let dir = copy_fixture();
    let mut extra = String::new();
    for name in ["a", "b", "c", "d"] {
        extra.push_str(&format!("local function {name}(x) if x then print(1) end if x then print(2) end if x then print(3) end if x then print(4) end end\n"));
    }
    extra.push_str("return {a=a,b=b,c=c,d=d}\n");
    std::fs::write(dir.path().join("medium.luau"), extra).unwrap();
    let main = std::fs::read_to_string(dir.path().join("main.luau")).unwrap();
    std::fs::write(
        dir.path().join("main.luau"),
        format!("local m = require('./medium')\nprint(m)\n{main}"),
    )
    .unwrap();
    let health = report(dir.path(), &["health"]);
    let targets = health["targets"].as_array().unwrap();
    let risky = targets
        .iter()
        .position(|t| t["path"] == "risky.luau")
        .unwrap();
    let medium = targets
        .iter()
        .position(|t| t["path"] == "medium.luau")
        .unwrap();
    assert!(risky < medium);
    assert!(
        targets[risky]["priority"].as_f64().unwrap()
            < targets[medium]["priority"].as_f64().unwrap()
    );
    assert_eq!(targets[medium]["effort"], "medium");
}
