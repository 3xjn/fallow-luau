//! `trace` — callers / callees of a returned module key through the require graph.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::Path;

use serde::Serialize;

use crate::discover::discover_files;
use crate::graph::{adjacency, build_require_graph, display_rel};
use crate::dead_code::analyze_dead_code;
use crate::dead_code::DeadCodeOptions;

#[derive(Debug, Clone, Serialize)]
pub struct TraceEdge {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceReport {
    pub schema_version: u32,
    pub root: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub depth: usize,
    pub callers: Vec<TraceEdge>,
    pub callees: Vec<TraceEdge>,
    pub caller_files: Vec<String>,
    pub callee_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

pub fn trace_symbol(
    root: &Path,
    path: &str,
    key: Option<&str>,
    depth: usize,
    explain: bool,
) -> Result<TraceReport, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    let files = discover_files(&root);
    let graph = build_require_graph(&root, &files)?;
    let rel = path.replace('\\', "/");
    if !graph.files.iter().any(|f| f == &rel) {
        // try resolve
        let abs = root.join(&rel);
        let rel = if abs.exists() {
            display_rel(&root, &abs)
        } else {
            return Err(format!("unknown file `{path}`"));
        };
        return trace_symbol(&root, &rel, key, depth, explain);
    }

    let adj = adjacency(&graph);
    let mut reverse: HashMap<String, Vec<String>> = HashMap::new();
    for (from, tos) in &adj {
        for to in tos {
            reverse.entry(to.clone()).or_default().push(from.clone());
        }
    }

    let depth = depth.max(1);
    let mut callers = Vec::new();
    let mut caller_files = BTreeSet::new();
    {
        let mut q = VecDeque::new();
        q.push_back((rel.clone(), 0usize));
        let mut seen = BTreeSet::from([rel.clone()]);
        while let Some((node, d)) = q.pop_front() {
            if d >= depth {
                continue;
            }
            if let Some(preds) = reverse.get(&node) {
                for p in preds {
                    callers.push(TraceEdge {
                        from: p.clone(),
                        to: node.clone(),
                        key: key.map(|k| k.to_string()),
                    });
                    caller_files.insert(p.clone());
                    if seen.insert(p.clone()) {
                        q.push_back((p.clone(), d + 1));
                    }
                }
            }
        }
    }

    let mut callees = Vec::new();
    let mut callee_files = BTreeSet::new();
    {
        let mut q = VecDeque::new();
        q.push_back((rel.clone(), 0usize));
        let mut seen = BTreeSet::from([rel.clone()]);
        while let Some((node, d)) = q.pop_front() {
            if d >= depth {
                continue;
            }
            if let Some(nexts) = adj.get(&node) {
                for n in nexts {
                    callees.push(TraceEdge {
                        from: node.clone(),
                        to: n.clone(),
                        key: None,
                    });
                    callee_files.insert(n.clone());
                    if seen.insert(n.clone()) {
                        q.push_back((n.clone(), d + 1));
                    }
                }
            }
        }
    }

    // If a key is named, note whether dead-code thinks it's unused.
    let _ = analyze_dead_code(
        &root,
        &files,
        &graph,
        &DeadCodeOptions {
            explain: false,
            ..Default::default()
        },
    );

    Ok(TraceReport {
        schema_version: 1,
        root: root.display().to_string(),
        path: rel,
        key: key.map(|k| k.to_string()),
        depth,
        callers,
        callees,
        caller_files: caller_files.into_iter().collect(),
        callee_files: callee_files.into_iter().collect(),
        _meta: if explain {
            Some(serde_json::json!({
                "docs": "https://docs.fallow.tools/cli/trace",
                "note": "Bounded require-graph walk. Key filtering is advisory; edges are module-level."
            }))
        } else {
            None
        },
    })
}
