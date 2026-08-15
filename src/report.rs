use serde_json::{json, Value};

use crate::explain::known_rules;
use crate::graph::RequireGraph;
use crate::meta::schema_meta;

pub fn schema_manifest(explain: bool) -> Value {
    let mut v = json!({
        "name": "fallow-luau",
        "manifest_version": "1",
        "schema_version": 1,
        "version": env!("CARGO_PKG_VERSION"),
        "parity_doc": "docs/parity.md",
        "commands": [
            { "name": "schema", "description": "Dump the capability manifest as JSON", "status": "done" },
            { "name": "list", "description": "List discovered Luau files and the require graph", "status": "done" },
            { "name": "health", "description": "Complexity, file scores, hotspots, targets", "status": "done" },
            { "name": "dead-code", "description": "Unused files, returned keys, locals, require cycles", "status": "done" },
            { "name": "dupes", "description": "Token/suffix-array clones across .lua/.luau", "status": "done" },
            { "name": "audit", "description": "Combined dead-code + health + dupes with verdict", "status": "done" },
            { "name": "explain", "description": "Rule docs for one issue type", "status": "done" },
            { "name": "mcp", "description": "stdio MCP server wrapping the same library", "status": "done" },
            { "name": "inspect", "description": "Compose evidence for one file or returned key", "status": "todo" },
            { "name": "trace", "description": "Callers/callees of a returned key", "status": "todo" },
            { "name": "watch", "description": "Re-run on file change", "status": "todo" },
            { "name": "init", "description": "Emit fallow-luau config", "status": "todo" },
            { "name": "config", "description": "Print resolved config", "status": "todo" },
            { "name": "suppressions", "description": "Inventory ignore markers", "status": "todo" },
            { "name": "report", "description": "Re-render saved JSON", "status": "todo" },
            { "name": "flags", "description": "Feature/settings gates via plugins", "status": "todo" },
            { "name": "viz", "description": "HTML treemap + require graph", "status": "todo" }
        ],
        "issue_types": known_rules(),
        "mcp_tools": [
            "schema", "list_project", "check_health", "find_dead_code",
            "find_dupes", "audit", "explain"
        ],
        "parser": "full_moon (luau)",
        "require_resolution": {
            "core": "string-literal require only",
            "unresolved": ["dynamic require", "loadstring", "load"],
            "plugins_optional": ["rojo paths", "custom import wrappers"]
        },
        "skip": [
            "css", "npm", "typescript-checker", "knip", "jscpd-migrate",
            "fallow-cloud", "react-next-vite", "node-napi"
        ],
        "status": "steps-1-5"
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
