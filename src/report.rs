use serde_json::{json, Value};

use crate::graph::RequireGraph;
use crate::meta::schema_meta;

pub fn schema_manifest(explain: bool) -> Value {
    let mut v = json!({
        "name": "fallow-luau",
        "schema_version": 1,
        "commands": [
            {
                "name": "schema",
                "description": "Dump the capability manifest as JSON"
            },
            {
                "name": "list",
                "description": "List discovered Luau files, require edges, and unresolved dynamics"
            },
            {
                "name": "health",
                "description": "Complexity, file scores, SIG profiles, hotspots, and refactoring targets"
            }
        ],
        "planned": ["dead-code", "dupes", "audit", "mcp", "inspect", "trace", "explain", "watch", "flags"],
        "parser": "full_moon (luau)",
        "require_resolution": {
            "core": "string-literal require only",
            "unresolved": ["dynamic require", "loadstring", "load"],
            "plugins_optional": ["rojo paths", "custom import wrappers"]
        },
        "health_metrics": [
            "cyclomatic",
            "cognitive",
            "complexity_density",
            "maintainability_index",
            "unit_size",
            "unit_interfacing",
            "crap_static_estimated",
            "hotspots",
            "targets"
        ],
        "status": "step-2-health"
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
                "require": "Core resolves require(\"...\") string literals to .lua/.luau/init modules. Dynamic require and loadstring are unresolved edges, not reachability."
            }),
        );
    }
    v
}
