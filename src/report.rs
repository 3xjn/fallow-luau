use serde_json::{json, Value};

use crate::explain::known_rules;
use crate::graph::RequireGraph;
use crate::meta::schema_meta;

pub fn schema_manifest(explain: bool) -> Value {
    let done = |name: &str, description: &str| {
        json!({ "name": name, "description": description, "status": "done" })
    };
    let mut v = json!({
        "name": "fallow-luau",
        "manifest_version": "1",
        "schema_version": 1,
        "version": env!("CARGO_PKG_VERSION"),
        "parity_doc": "docs/parity.md",
        "commands": [
            done("schema", "Dump the capability manifest as JSON"),
            done("list", "List discovered Luau files and the require graph"),
            done("health", "Complexity, file scores, hotspots, targets, score"),
            done("dead-code", "Unused files, returned keys, locals, types, cycles"),
            done("dupes", "Token/suffix-array clones across .lua/.luau"),
            done("audit", "Combined dead-code + health + dupes with verdict"),
            done("explain", "Rule docs for one issue type"),
            done("inspect", "Compose evidence for one file or returned key"),
            done("trace", "Callers/callees through the require graph"),
            done("watch", "Re-run audit when files change"),
            done("init", "Emit fallow-luau config"),
            done("config", "Print resolved config"),
            done("suppressions", "Inventory ignore markers"),
            done("report", "Re-render saved JSON"),
            done("flags", "Feature/settings gate detection"),
            done("viz", "HTML treemap + require graph"),
            done("mcp", "stdio MCP server wrapping the same library")
        ],
        "issue_types": known_rules(),
        "mcp_tools": [
            "schema", "list_project", "check_health", "find_dead_code",
            "find_dupes", "audit", "explain", "inspect", "trace", "flags"
        ],
        "parser": "full_moon (luau)",
        "require_resolution": {
            "core": "string-literal require only",
            "unresolved": ["dynamic require", "loadstring", "load"],
            "plugins_optional": ["rojo paths", "custom import wrappers"]
        },
        "skip": [
            "css", "npm", "typescript-checker", "knip", "jscpd-migrate",
            "fallow-cloud", "react-next-vite", "node-napi", "fix"
        ],
        "status": "parity-complete"
    });
    if explain {
        v.as_object_mut()
            .unwrap()
            .insert("_meta".into(), schema_meta());
    }
    v
}

pub fn list_report(root: &str, graph: &RequireGraph, explain: bool) -> Value {
    let mut v = json!({
        "schema_version": 1,
        "root": root,
        "files": graph.files,
        "file_count": graph.files.len(),
        "edges": graph.edges,
        "edge_count": graph.edges.len(),
        "unresolved": graph.unresolved,
        "unresolved_count": graph.unresolved.len(),
    });
    if explain {
        v.as_object_mut().unwrap().insert(
            "_meta".into(),
            json!({
                "docs": "https://docs.fallow.tools/cli/list",
                "require": "Core resolves require(\"...\") string literals to .lua/.luau/init modules. Dynamic require and loadstring are unresolved edges, not reachability.",
                "parity": "docs/parity.md"
            }),
        );
    }
    v
}
