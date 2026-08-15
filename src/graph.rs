use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use full_moon::ast::{Call, Expression, FunctionArgs, Prefix, Suffix};
use full_moon::node::Node;
use full_moon::visitors::Visitor;
use full_moon::parse;

use crate::resolve::{resolve_require, ResolvedRequire};

/// How a require edge was classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RequireKind {
    /// `require("...")` with a string literal — resolved or missing.
    StringLiteral,
    /// Dynamic `require(expr)` / non-literal — unresolved edge, not reachability.
    Dynamic,
    /// `loadstring(...)` or similar dynamic loader.
    Loadstring,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RequireEdge {
    pub from: String,
    pub kind: RequireKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub specifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RequireGraph {
    pub files: Vec<String>,
    pub edges: Vec<RequireEdge>,
    pub unresolved: Vec<RequireEdge>,
}

/// Build the require graph for the given absolute file paths (rooted for relative display).
pub fn build_require_graph(root: &Path, files: &[PathBuf]) -> Result<RequireGraph, String> {
    let mut abs_to_rel: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut rel_files = Vec::new();
    for file in files {
        let abs = file.canonicalize().unwrap_or_else(|_| file.clone());
        let rel = display_rel(root, &abs);
        abs_to_rel.insert(abs.clone(), rel.clone());
        rel_files.push(rel);
    }
    rel_files.sort();
    rel_files.dedup();

    let mut edges = Vec::new();
    let mut unresolved = Vec::new();

    for file in files {
        let abs = file.canonicalize().unwrap_or_else(|_| file.clone());
        let from_rel = abs_to_rel
            .get(&abs)
            .cloned()
            .unwrap_or_else(|| display_rel(root, &abs));
        let source = std::fs::read_to_string(file)
            .map_err(|e| format!("read {}: {e}", file.display()))?;
        let ast = parse(&source).map_err(|errs| {
            format!(
                "parse {}: {}",
                file.display(),
                errs.iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        })?;

        let mut visitor = RequireVisitor::default();
        visitor.visit_ast(&ast);

        for hit in visitor.hits {
            match hit.kind {
                RequireHitKind::StringLiteral(spec) => {
                    let line = hit.line;
                    match resolve_require(&abs, &spec) {
                        ResolvedRequire::Found(to_abs) => {
                            let to_rel = abs_to_rel
                                .get(&to_abs)
                                .cloned()
                                .unwrap_or_else(|| display_rel(root, &to_abs));
                            edges.push(RequireEdge {
                                from: from_rel.clone(),
                                kind: RequireKind::StringLiteral,
                                specifier: Some(spec),
                                to: Some(to_rel),
                                line,
                            });
                        }
                        ResolvedRequire::Missing { specifier } => {
                            let edge = RequireEdge {
                                from: from_rel.clone(),
                                kind: RequireKind::StringLiteral,
                                specifier: Some(specifier),
                                to: None,
                                line,
                            };
                            unresolved.push(edge.clone());
                            edges.push(edge);
                        }
                    }
                }
                RequireHitKind::Dynamic => {
                    let edge = RequireEdge {
                        from: from_rel.clone(),
                        kind: RequireKind::Dynamic,
                        specifier: None,
                        to: None,
                        line: hit.line,
                    };
                    unresolved.push(edge.clone());
                    edges.push(edge);
                }
                RequireHitKind::Loadstring => {
                    let edge = RequireEdge {
                        from: from_rel.clone(),
                        kind: RequireKind::Loadstring,
                        specifier: None,
                        to: None,
                        line: hit.line,
                    };
                    unresolved.push(edge.clone());
                    edges.push(edge);
                }
            }
        }
    }

    edges.sort_by(|a, b| {
        (&a.from, a.line, &a.specifier).cmp(&(&b.from, b.line, &b.specifier))
    });
    unresolved.sort_by(|a, b| (&a.from, a.line, a.kind).cmp(&(&b.from, b.line, b.kind)));

    Ok(RequireGraph {
        files: rel_files,
        edges,
        unresolved,
    })
}

pub fn display_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Default)]
struct RequireVisitor {
    hits: Vec<RequireHit>,
}

struct RequireHit {
    kind: RequireHitKind,
    line: usize,
}

enum RequireHitKind {
    StringLiteral(String),
    Dynamic,
    Loadstring,
}

impl Visitor for RequireVisitor {
    fn visit_function_call(&mut self, call: &full_moon::ast::FunctionCall) {
        if let Some(hit) = classify_call(call) {
            self.hits.push(hit);
        }
    }
}

fn classify_call(call: &full_moon::ast::FunctionCall) -> Option<RequireHit> {
    let name = match call.prefix() {
        Prefix::Name(token) => token.token().to_string(),
        _ => return None,
    };
    let name = name.trim();
    let suffixes: Vec<_> = call.suffixes().collect();
    if suffixes.is_empty() {
        return None;
    }
    // require(...) or loadstring(...) as a bare call.
    let Suffix::Call(Call::AnonymousCall(args)) = suffixes[0] else {
        return None;
    };
    let line = call
        .prefix()
        .start_position()
        .map(|p| p.line())
        .unwrap_or(1);

    if name == "require" {
        return Some(match first_string_arg(args) {
            Some(spec) => RequireHit {
                kind: RequireHitKind::StringLiteral(unquote(&spec)),
                line,
            },
            None => RequireHit {
                kind: RequireHitKind::Dynamic,
                line,
            },
        });
    }
    if name == "loadstring" || name == "load" {
        return Some(RequireHit {
            kind: RequireHitKind::Loadstring,
            line,
        });
    }
    None
}

fn first_string_arg(args: &FunctionArgs) -> Option<String> {
    match args {
        FunctionArgs::Parentheses { arguments, .. } => {
            let first = arguments.iter().next()?;
            match first {
                Expression::String(token) => Some(token.token().to_string()),
                Expression::Parentheses { expression, .. } => match expression.as_ref() {
                    Expression::String(token) => Some(token.token().to_string()),
                    _ => None,
                },
                _ => None,
            }
        }
        FunctionArgs::String(token) => Some(token.token().to_string()),
        _ => None,
    }
}

fn unquote(raw: &str) -> String {
    let t = raw.trim();
    if t.len() >= 2 {
        let bytes = t.as_bytes();
        let q = bytes[0];
        if (q == b'"' || q == b'\'') && bytes[t.len() - 1] == q {
            return t[1..t.len() - 1].to_string();
        }
        // long string [[...]]
        if t.starts_with('[') {
            if let Some(end) = t.rfind(']') {
                let inner_start = t.find('[').and_then(|i| {
                    let rest = &t[i + 1..];
                    if rest.starts_with('[') {
                        Some(i + 2)
                    } else if let Some(eq) = rest.find('[') {
                        Some(i + 1 + eq + 1)
                    } else {
                        None
                    }
                });
                if let Some(start) = inner_start {
                    if end > start {
                        return t[start..end].to_string();
                    }
                }
            }
        }
    }
    t.to_string()
}

/// Adjacency: from → set of resolved `to` paths (relative).
pub fn adjacency(graph: &RequireGraph) -> BTreeMap<String, BTreeSet<String>> {
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for f in &graph.files {
        map.entry(f.clone()).or_default();
    }
    for e in &graph.edges {
        if let Some(to) = &e.to {
            map.entry(e.from.clone()).or_default().insert(to.clone());
        }
    }
    map
}

/// Fan-in / fan-out counts from resolved edges only.
pub fn fan_counts(graph: &RequireGraph) -> BTreeMap<String, (usize, usize)> {
    let adj = adjacency(graph);
    let mut fan_in: BTreeMap<String, usize> = BTreeMap::new();
    let mut fan_out: BTreeMap<String, usize> = BTreeMap::new();
    for f in &graph.files {
        fan_in.insert(f.clone(), 0);
        fan_out.insert(f.clone(), 0);
    }
    for (from, tos) in &adj {
        *fan_out.entry(from.clone()).or_default() = tos.len();
        for to in tos {
            *fan_in.entry(to.clone()).or_default() += 1;
        }
    }
    graph
        .files
        .iter()
        .map(|f| {
            (
                f.clone(),
                (
                    *fan_in.get(f).unwrap_or(&0),
                    *fan_out.get(f).unwrap_or(&0),
                ),
            )
        })
        .collect()
}
