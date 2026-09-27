//! `trace` — callers / callees of a returned module key through the require graph.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::Path;

use serde::Serialize;

use crate::discover::discover_configured_files;
use crate::graph::{adjacency, build_require_graph};

#[derive(Debug, Clone, Serialize)]
pub struct TraceEdge {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
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
    pub scope: String,
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
    let files = discover_configured_files(&root)?;
    let graph = build_require_graph(&root, &files)?;
    let rel = crate::inspect::normalize_target(&root, path, &graph.files)?;
    let references = crate::references::collect(&root, &files, &graph)?;
    if let Some(key) = key {
        crate::inspect::inspect_target(&root, &rel, Some(key), false)?;
    }

    let adj = adjacency(&graph);
    let mut reverse: HashMap<String, Vec<String>> = HashMap::new();
    for (from, tos) in &adj {
        for to in tos {
            reverse.entry(to.clone()).or_default().push(from.clone());
        }
    }

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
                    if d == 0 && key.is_some() {
                        let hits: Vec<_> = references
                            .iter()
                            .filter(|r| r.from == *p && r.to == node && r.key.as_deref() == key)
                            .collect();
                        if hits.is_empty() {
                            continue;
                        }
                        for hit in hits {
                            callers.push(TraceEdge {
                                from: p.clone(),
                                to: node.clone(),
                                key: hit.key.clone(),
                                line: Some(hit.line),
                            });
                        }
                    } else {
                        callers.push(TraceEdge {
                            from: p.clone(),
                            to: node.clone(),
                            key: None,
                            line: None,
                        });
                    }
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
                        line: None,
                    });
                    callee_files.insert(n.clone());
                    if seen.insert(n.clone()) {
                        q.push_back((n.clone(), d + 1));
                    }
                }
            }
        }
    }

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
        scope: "Direct callers are literal imported-key references when a key is supplied; transitive callers and all callees are module dependencies. Dynamic references and same-module calls are not resolved.".into(),
        _meta: if explain {
            Some(serde_json::json!({
                "docs": "https://docs.fallow.tools/cli/trace",
                "note": "Keyed direct callers include observed source lines; other edges are module-level."
            }))
        } else {
            None
        },
    })
}
