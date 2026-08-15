//! Minimal stdio MCP server wrapping the same library as the CLI.
//! Always includes `_meta` on tool results.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::audit::{analyze_audit, AuditOptions};
use crate::dead_code::{analyze_dead_code, DeadCodeOptions};
use crate::discover::discover_files;
use crate::dupes::{analyze_dupes, DupesOptions};
use crate::explain::explain_rule;
use crate::graph::build_require_graph;
use crate::project::{analyze_project, ProjectOptions};
use crate::report::{list_report, schema_manifest};
use crate::health::HealthOptions;

pub fn run_mcp_stdio() -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut stdout = std::io::stdout().lock();

    loop {
        let msg = read_message(&mut reader)?;
        let Some(msg) = msg else {
            break;
        };
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = msg.get("id").cloned();
        let params = msg.get("params").cloned().unwrap_or(json!({}));

        // Notifications have no id — acknowledge silently except initialized.
        if id.is_none() {
            continue;
        }

        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "fallow-luau",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            "tools/list" => Ok(json!({ "tools": tool_defs() })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match call_tool(name, args) {
                    Ok(value) => Ok(json!({
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).unwrap() }],
                        "structuredContent": value
                    })),
                    Err(e) => Ok(json!({
                        "isError": true,
                        "content": [{ "type": "text", "text": e }]
                    })),
                }
            }
            "ping" => Ok(json!({ })),
            _ => Err(format!("method not found: {method}")),
        };

        let response = match result {
            Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": e }
            }),
        };
        write_message(&mut stdout, &response)?;
    }
    Ok(())
}

fn tool_defs() -> Vec<Value> {
    vec![
        tool("schema", "Capability manifest (commands, issue types, parity status)"),
        tool("list_project", "Discovered files and string-literal require graph"),
        tool("check_health", "Complexity, MI, hotspots, targets"),
        tool("find_dead_code", "Unused files, returned keys, locals, require cycles"),
        tool("find_dupes", "Token/suffix-array clones across .lua/.luau"),
        tool("audit", "Combined dead-code + health + dupes with pass/warn/fail"),
        tool("explain", "Rule docs for one issue type id"),
    ]
}

fn tool(name: &str, description: &str) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": {
                "root": { "type": "string", "description": "Project root (default .)" },
                "id": { "type": "string", "description": "Issue type id for explain" },
                "changed_since": { "type": "string", "description": "Git ref for audit scoping" }
            }
        }
    })
}

fn call_tool(name: &str, args: Value) -> Result<Value, String> {
    let root = PathBuf::from(args.get("root").and_then(|r| r.as_str()).unwrap_or("."));
    match name {
        "schema" => Ok(schema_manifest(true)),
        "list_project" => {
            let root = root.canonicalize().map_err(|e| e.to_string())?;
            let files = discover_files(&root);
            let graph = build_require_graph(&root, &files)?;
            Ok(list_report(&root.display().to_string(), &graph, true))
        }
        "check_health" => {
            let (_p, report) = analyze_project(
                &root,
                &ProjectOptions {
                    health: HealthOptions {
                        explain: true,
                        ..HealthOptions::default()
                    },
                },
            )?;
            Ok(serde_json::to_value(report).unwrap())
        }
        "find_dead_code" => {
            let root = root.canonicalize().map_err(|e| e.to_string())?;
            let files = discover_files(&root);
            let graph = build_require_graph(&root, &files)?;
            let report = analyze_dead_code(
                &root,
                &files,
                &graph,
                &DeadCodeOptions {
                    explain: true,
                    ..DeadCodeOptions::default()
                },
            )?;
            Ok(serde_json::to_value(report).unwrap())
        }
        "find_dupes" => {
            let root = root.canonicalize().map_err(|e| e.to_string())?;
            let files = discover_files(&root);
            let report = analyze_dupes(
                &root,
                &files,
                &DupesOptions {
                    explain: true,
                    ..DupesOptions::default()
                },
            )?;
            Ok(serde_json::to_value(report).unwrap())
        }
        "audit" => {
            let changed = args
                .get("changed_since")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string());
            let report = analyze_audit(
                &root,
                &AuditOptions {
                    explain: true,
                    changed_since: changed,
                    ..AuditOptions::default()
                },
            )?;
            Ok(serde_json::to_value(report).unwrap())
        }
        "explain" => {
            let id = args.get("id").and_then(|i| i.as_str()).unwrap_or("");
            Ok(explain_rule(id))
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        headers.push_str(&line);
    }
    let mut content_length = None;
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_length = Some(
                rest.trim()
                    .parse::<usize>()
                    .map_err(|e| format!("bad content-length: {e}"))?,
            );
        }
    }
    let len = content_length.ok_or_else(|| "missing Content-Length".to_string())?;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_slice(&buf).map_err(|e| e.to_string())?;
    Ok(Some(v))
}

fn write_message(out: &mut impl Write, value: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    write!(out, "Content-Length: {}\r\n\r\n", body.len()).map_err(|e| e.to_string())?;
    out.write_all(&body).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(())
}
