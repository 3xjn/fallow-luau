use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fallow-luau"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

fn report(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn returned_code_is_scored_and_returned_locals_are_live() {
    let root = fixture("returns");
    let inspect = report(cli(&root, &["inspect", "main.luau"]));
    let functions = inspect["functions"].as_array().unwrap();
    assert_eq!(functions.len(), 2);
    let choose = functions.iter().find(|f| f["name"] == "choose").unwrap();
    assert_eq!(choose["cyclomatic"], 3);
    let anonymous = functions
        .iter()
        .find(|f| f["name"] == "<anonymous>")
        .unwrap();
    assert_eq!(anonymous["cyclomatic"], 2);
    let dead = report(cli(&root, &["dead-code"]));
    assert_eq!(dead["unused_locals"], json!([]));
    assert_eq!(dead["unused_exports"], json!([]));
}

#[test]
fn used_type_is_not_reported_unused() {
    let dead = report(cli(&fixture("types"), &["dead-code"]));
    let names: Vec<_> = dead["unused_types"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["NeverUsed"]);
}

#[test]
fn duplicate_lines_are_a_union_of_reported_ranges() {
    let dupes = report(cli(&fixture("dupes"), &["dupes"]));
    let mut lines = std::collections::BTreeSet::new();
    for group in dupes["clone_groups"].as_array().unwrap() {
        for instance in group["instances"].as_array().unwrap() {
            for line in
                instance["start_line"].as_u64().unwrap()..=instance["end_line"].as_u64().unwrap()
            {
                lines.insert((instance["path"].as_str().unwrap(), line));
            }
        }
    }
    assert!(!lines.is_empty());
    assert_eq!(
        dupes["stats"]["duplicated_lines"].as_u64().unwrap(),
        lines.len() as u64
    );
    assert!(dupes["stats"]["duplication_percentage"].as_f64().unwrap() <= 100.0);
}

#[test]
fn config_controls_discovery_entries_and_thresholds_with_cli_override() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("app.luau"),
        include_str!("fixtures/returns/main.luau"),
    )
    .unwrap();
    std::fs::write(dir.path().join("main.luau"), "return {}").unwrap();
    std::fs::create_dir(dir.path().join("vendor")).unwrap();
    std::fs::write(
        dir.path().join("vendor/ignored.luau"),
        "this is invalid syntax",
    )
    .unwrap();
    std::fs::write(
        dir.path().join(".fallow-luau.json"),
        json!({
            "entry": ["app.*"], "ignore": ["vendor/**"], "health": {"max_cyclomatic": 2}
        })
        .to_string(),
    )
    .unwrap();
    let dead = report(cli(dir.path(), &["dead-code"]));
    assert_eq!(dead["entry_points"], json!(["app.luau"]));
    assert_eq!(dead["unused_files"], json!(["main.luau"]));
    let health = report(cli(dir.path(), &["health"]));
    assert_eq!(health["files_analyzed"], 2);
    assert_eq!(health["findings"].as_array().unwrap().len(), 2);
    let overridden = report(cli(dir.path(), &["health", "--max-cyclomatic", "20"]));
    assert_eq!(overridden["findings"], json!([]));
    let audit = cli(dir.path(), &["audit"]);
    assert!(!audit.status.success());
    let audit: Value = serde_json::from_slice(&audit.stdout).unwrap();
    assert_eq!(audit["verdict"], "fail");
    assert_eq!(audit["health"]["findings"], health["findings"]);
}

#[test]
fn init_module_self_and_dotted_sibling_requires_resolve() {
    let graph = report(cli(&fixture("module_paths"), &["list"]));
    assert_eq!(graph["unresolved_count"], 0);
    assert_eq!(graph["edge_count"], 3);
    let edges = graph["edges"].as_array().unwrap();
    assert!(edges.iter().any(|e| e["to"] == "package/child.luau"));
    assert!(edges.iter().any(|e| e["to"] == "sibling.spec.luau"));
}

#[test]
fn long_string_requires_resolve() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.luau"),
        "local a = require [[./util]]\nreturn require [=[./util]=]",
    )
    .unwrap();
    std::fs::write(dir.path().join("util.luau"), "return {}").unwrap();
    let graph = report(cli(dir.path(), &["list"]));
    assert_eq!(graph["unresolved_count"], 0);
    assert_eq!(graph["edge_count"], 2);
}

#[test]
fn mcp_accepts_ndjson_and_matches_cli_health() {
    let root = fixture("dead_code");
    let mut child = Command::new(env!("CARGO_BIN_EXE_fallow-luau"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"check_health","arguments":{"root":root}}}),
    ];
    let mut input = child.stdin.take().unwrap();
    for message in messages {
        writeln!(input, "{message}").unwrap();
    }
    drop(input);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    let expected = report(cli(&root, &["health", "--explain"]));
    assert_eq!(responses[1]["result"]["structuredContent"], expected);
}
