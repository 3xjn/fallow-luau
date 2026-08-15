//! Dead-code analysis for Luau (SPEC step 3).
//!
//! Adaptations from Fallow dead-code:
//! - unused file = unreachable from entry points
//! - unused export = unused key on a returned module table
//! - unused local = binding never read (nested functions included)
//! - cycles = require-graph SCCs (Tarjan), no depth limit
//!
//! Docs: https://docs.fallow.tools/explanations/dead-code

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use full_moon::ast::{
    Call, Expression, Field, FunctionArgs, Prefix, Stmt, Suffix, Var,
};
use full_moon::node::Node;
use full_moon::parse;
use serde::Serialize;

use crate::discover::is_test_path;
use crate::graph::{adjacency, display_rel, RequireGraph};
use crate::meta::dead_code_meta;

#[derive(Debug, Clone, Serialize)]
pub struct DeadCodeFinding {
    pub path: String,
    pub kind: DeadKind,
    pub name: String,
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadKind {
    UnusedFile,
    UnusedExport,
    UnusedLocal,
    CircularDependency,
}

#[derive(Debug, Clone, Serialize)]
pub struct CycleInfo {
    pub path: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeadCodeReport {
    pub schema_version: u32,
    pub root: String,
    pub entry_points: Vec<String>,
    pub unused_files: Vec<String>,
    pub unused_exports: Vec<DeadCodeFinding>,
    pub unused_locals: Vec<DeadCodeFinding>,
    pub cycles: Vec<CycleInfo>,
    pub findings: Vec<DeadCodeFinding>,
    /// path → dead_code_ratio (unused returned keys / total returned keys)
    pub dead_ratio_by_file: BTreeMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default)]
pub struct DeadCodeOptions {
    pub explain: bool,
    /// Extra entry globs / relative paths
    pub extra_entries: Vec<String>,
}

pub fn analyze_dead_code(
    root: &Path,
    files: &[PathBuf],
    graph: &RequireGraph,
    opts: &DeadCodeOptions,
) -> Result<DeadCodeReport, String> {
    let root = root
        .canonicalize()
        .unwrap_or_else(|_| root.to_path_buf());
    let entries = detect_entry_points(&root, files, graph, &opts.extra_entries);
    let reachable = reachability(graph, &entries);
    let mut unused_files: Vec<String> = graph
        .files
        .iter()
        .filter(|f| !reachable.contains(*f))
        .cloned()
        .collect();
    unused_files.sort();

    let mut modules: BTreeMap<String, ModuleInfo> = BTreeMap::new();
    for file in files {
        let rel = display_rel(&root, file);
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
        modules.insert(rel, extract_module_info(&ast));
    }

    // Cross-file key references: require("x").key / local m = require("x"); m.key
    let key_refs = collect_key_references(&root, files, graph)?;

    let mut unused_exports = Vec::new();
    let mut dead_ratio_by_file = BTreeMap::new();
    for (path, info) in &modules {
        if info.exports.is_empty() {
            dead_ratio_by_file.insert(path.clone(), 0.0);
            continue;
        }
        let refs = key_refs.get(path).cloned().unwrap_or_default();
        let mut unused = 0usize;
        for (name, line) in &info.exports {
            // Entry modules and test files keep public surface unless never self-used
            // and never referenced — still report if zero refs from anywhere.
            if !refs.contains(name) {
                unused += 1;
                unused_exports.push(DeadCodeFinding {
                    path: path.clone(),
                    kind: DeadKind::UnusedExport,
                    name: name.clone(),
                    line: *line,
                    message: format!("returned key `{name}` is never referenced"),
                });
            }
        }
        dead_ratio_by_file.insert(
            path.clone(),
            unused as f64 / info.exports.len() as f64,
        );
    }
    unused_exports.sort_by(|a, b| (&a.path, a.line, &a.name).cmp(&(&b.path, b.line, &b.name)));

    let mut unused_locals = Vec::new();
    for (path, info) in &modules {
        for local in &info.unused_locals {
            unused_locals.push(DeadCodeFinding {
                path: path.clone(),
                kind: DeadKind::UnusedLocal,
                name: local.name.clone(),
                line: local.line,
                message: format!("local `{}` is never read", local.name),
            });
        }
    }
    unused_locals.sort_by(|a, b| (&a.path, a.line, &a.name).cmp(&(&b.path, b.line, &b.name)));

    let cycles = find_cycles(graph);
    let mut findings = Vec::new();
    for f in &unused_files {
        findings.push(DeadCodeFinding {
            path: f.clone(),
            kind: DeadKind::UnusedFile,
            name: f.clone(),
            line: 1,
            message: format!("file not reachable from entry points"),
        });
    }
    findings.extend(unused_exports.clone());
    findings.extend(unused_locals.clone());
    for c in &cycles {
        let tip = c.path.first().cloned().unwrap_or_default();
        findings.push(DeadCodeFinding {
            path: tip,
            kind: DeadKind::CircularDependency,
            name: c.path.join(" → "),
            line: 1,
            message: format!("require cycle: {}", c.path.join(" → ")),
        });
    }

    Ok(DeadCodeReport {
        schema_version: 1,
        root: root.display().to_string(),
        entry_points: entries,
        unused_files,
        unused_exports,
        unused_locals,
        cycles,
        findings,
        dead_ratio_by_file,
        _meta: if opts.explain {
            Some(dead_code_meta())
        } else {
            None
        },
    })
}

fn detect_entry_points(
    root: &Path,
    files: &[PathBuf],
    graph: &RequireGraph,
    extra: &[String],
) -> Vec<String> {
    let mut entries = BTreeSet::new();
    for file in files {
        let rel = display_rel(root, file);
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        if name == "init.lua"
            || name == "init.luau"
            || name == "main.lua"
            || name == "main.luau"
            || name.starts_with("main.")
        {
            entries.insert(rel);
        }
    }
    for e in extra {
        let p = root.join(e);
        if p.exists() {
            entries.insert(display_rel(root, &p));
        } else {
            // treat as relative path already
            entries.insert(e.replace('\\', "/"));
        }
    }
    if entries.is_empty() {
        // Library-mode: every fan-in=0 non-test file is an entry (nothing to delete).
        let adj = adjacency(graph);
        let mut fan_in: HashMap<&str, usize> = HashMap::new();
        for f in &graph.files {
            fan_in.insert(f, 0);
        }
        for tos in adj.values() {
            for t in tos {
                *fan_in.entry(t.as_str()).or_default() += 1;
            }
        }
        for f in &graph.files {
            if fan_in.get(f.as_str()).copied().unwrap_or(0) == 0 && !is_test_path(Path::new(f)) {
                entries.insert(f.clone());
            }
        }
    }
    // Always include test files as soft entries so they don't show as unused-files
    // when nothing requires them (they require production code instead).
    for f in &graph.files {
        if is_test_path(Path::new(f)) {
            entries.insert(f.clone());
        }
    }
    entries.into_iter().collect()
}

fn reachability(graph: &RequireGraph, entries: &[String]) -> BTreeSet<String> {
    let adj = adjacency(graph);
    let mut seen = BTreeSet::new();
    let mut q = VecDeque::new();
    for e in entries {
        if graph.files.iter().any(|f| f == e) {
            q.push_back(e.clone());
            seen.insert(e.clone());
        }
    }
    while let Some(n) = q.pop_front() {
        if let Some(nexts) = adj.get(&n) {
            for t in nexts {
                if seen.insert(t.clone()) {
                    q.push_back(t.clone());
                }
            }
        }
    }
    seen
}

#[derive(Debug, Clone)]
struct ModuleInfo {
    exports: Vec<(String, usize)>,
    unused_locals: Vec<LocalBinding>,
}

#[derive(Debug, Clone)]
struct LocalBinding {
    name: String,
    line: usize,
}

fn extract_module_info(ast: &full_moon::ast::Ast) -> ModuleInfo {
    let mut exports = Vec::new();
    // Last return at file scope wins for module table.
    if let Some((_, ret)) = ast.nodes().stmts_with_semicolon().last() {
        // Block may end with return via last_stmt
        let _ = ret;
    }
    if let Some(last) = ast.nodes().last_stmt() {
        if let full_moon::ast::LastStmt::Return(r) = last {
            if let Some(expr) = r.returns().iter().next() {
                collect_table_keys(expr, &mut exports);
            }
        }
    }

    let unused_locals = find_unused_locals(ast);
    ModuleInfo {
        exports,
        unused_locals,
    }
}

fn collect_table_keys(expr: &Expression, out: &mut Vec<(String, usize)>) {
    match expr {
        Expression::TableConstructor(table) => {
            for field in table.fields() {
                match field {
                    Field::NameKey { key, .. } => {
                        let name = key.token().to_string();
                        let line = key.start_position().map(|p| p.line()).unwrap_or(1);
                        out.push((name, line));
                    }
                    Field::ExpressionKey { key, .. } => {
                        if let Expression::String(tok) = key {
                            let raw = tok.token().to_string();
                            let name = unquote_str(&raw);
                            let line = tok.start_position().map(|p| p.line()).unwrap_or(1);
                            out.push((name, line));
                        }
                    }
                    _ => {}
                }
            }
        }
        Expression::Parentheses { expression, .. } => collect_table_keys(expression, out),
        Expression::TypeAssertion { expression, .. } => collect_table_keys(expression, out),
        _ => {}
    }
}

fn unquote_str(raw: &str) -> String {
    let t = raw.trim();
    if t.len() >= 2 {
        let b = t.as_bytes();
        if (b[0] == b'"' || b[0] == b'\'') && b[t.len() - 1] == b[0] {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

/// Find locals that are never read. Conservative: skip `_` and ignore writes-as-reads.
fn find_unused_locals(ast: &full_moon::ast::Ast) -> Vec<LocalBinding> {
    let mut declared: Vec<(String, usize, usize)> = Vec::new(); // name, line, scope_id
    let mut reads: HashSet<String> = HashSet::new();
    let mut scope = 0usize;
    walk_block_locals(ast.nodes(), &mut declared, &mut reads, &mut scope);
    declared
        .into_iter()
        .filter(|(name, _, _)| {
            name != "_"
                && !name.starts_with('_')
                && !reads.contains(name)
        })
        .map(|(name, line, _)| LocalBinding { name, line })
        .collect()
}

fn walk_block_locals(
    block: &full_moon::ast::Block,
    declared: &mut Vec<(String, usize, usize)>,
    reads: &mut HashSet<String>,
    scope: &mut usize,
) {
    for stmt in block.stmts() {
        walk_stmt_locals(stmt, declared, reads, scope);
    }
}

fn walk_stmt_locals(
    stmt: &Stmt,
    declared: &mut Vec<(String, usize, usize)>,
    reads: &mut HashSet<String>,
    scope: &mut usize,
) {
    match stmt {
        Stmt::LocalAssignment(a) => {
            for name in a.names() {
                let n = name.token().to_string();
                let line = name.start_position().map(|p| p.line()).unwrap_or(1);
                declared.push((n, line, *scope));
            }
            for expr in a.expressions() {
                walk_expr_reads(expr, reads);
            }
        }
        Stmt::LocalFunction(f) => {
            let n = f.name().token().to_string();
            let line = f.name().start_position().map(|p| p.line()).unwrap_or(1);
            declared.push((n, line, *scope));
            *scope += 1;
            // parameters count as declared in inner scope — skip unused-param noise for now
            // by marking them read
            for p in f.body().parameters() {
                if let full_moon::ast::Parameter::Name(tok) = p {
                    reads.insert(tok.token().to_string());
                }
            }
            walk_block_locals(f.body().block(), declared, reads, scope);
        }
        Stmt::FunctionDeclaration(f) => {
            *scope += 1;
            for p in f.body().parameters() {
                if let full_moon::ast::Parameter::Name(tok) = p {
                    reads.insert(tok.token().to_string());
                }
            }
            walk_block_locals(f.body().block(), declared, reads, scope);
        }
        Stmt::Assignment(a) => {
            for expr in a.expressions() {
                walk_expr_reads(expr, reads);
            }
            // LHS names count as writes, not reads
        }
        Stmt::FunctionCall(call) => walk_call_reads(call, reads),
        Stmt::If(i) => {
            walk_expr_reads(i.condition(), reads);
            walk_block_locals(i.block(), declared, reads, scope);
            for e in i.else_if().into_iter().flatten() {
                walk_expr_reads(e.condition(), reads);
                walk_block_locals(e.block(), declared, reads, scope);
            }
            if let Some(b) = i.else_block() {
                walk_block_locals(b, declared, reads, scope);
            }
        }
        Stmt::While(w) => {
            walk_expr_reads(w.condition(), reads);
            walk_block_locals(w.block(), declared, reads, scope);
        }
        Stmt::Repeat(r) => {
            walk_block_locals(r.block(), declared, reads, scope);
            walk_expr_reads(r.until(), reads);
        }
        Stmt::NumericFor(f) => {
            reads.insert(f.index_variable().token().to_string());
            walk_expr_reads(f.start(), reads);
            walk_expr_reads(f.end(), reads);
            if let Some(s) = f.step() {
                walk_expr_reads(s, reads);
            }
            walk_block_locals(f.block(), declared, reads, scope);
        }
        Stmt::GenericFor(f) => {
            for n in f.names() {
                reads.insert(n.token().to_string());
            }
            for e in f.expressions() {
                walk_expr_reads(e, reads);
            }
            walk_block_locals(f.block(), declared, reads, scope);
        }
        Stmt::Do(d) => walk_block_locals(d.block(), declared, reads, scope),
        Stmt::CompoundAssignment(c) => walk_expr_reads(c.rhs(), reads),
        _ => {}
    }
}

fn walk_expr_reads(expr: &Expression, reads: &mut HashSet<String>) {
    match expr {
        Expression::Var(v) => walk_var_reads(v, reads),
        Expression::FunctionCall(c) => walk_call_reads(c, reads),
        Expression::BinaryOperator { lhs, rhs, .. } => {
            walk_expr_reads(lhs, reads);
            walk_expr_reads(rhs, reads);
        }
        Expression::UnaryOperator { expression, .. } => walk_expr_reads(expression, reads),
        Expression::Parentheses { expression, .. } => walk_expr_reads(expression, reads),
        Expression::TypeAssertion { expression, .. } => walk_expr_reads(expression, reads),
        Expression::IfExpression(i) => {
            walk_expr_reads(i.condition(), reads);
            walk_expr_reads(i.if_expression(), reads);
            for e in i.else_if_expressions().into_iter().flatten() {
                walk_expr_reads(e.condition(), reads);
                walk_expr_reads(e.expression(), reads);
            }
            walk_expr_reads(i.else_expression(), reads);
        }
        Expression::TableConstructor(t) => {
            for f in t.fields() {
                match f {
                    Field::ExpressionKey { key, value, .. } => {
                        walk_expr_reads(key, reads);
                        walk_expr_reads(value, reads);
                    }
                    Field::NameKey { value, .. } => walk_expr_reads(value, reads),
                    Field::NoKey(v) => walk_expr_reads(v, reads),
                    _ => {}
                }
            }
        }
        Expression::Function(anon) => {
            for p in anon.body().parameters() {
                if let full_moon::ast::Parameter::Name(tok) = p {
                    reads.insert(tok.token().to_string());
                }
            }
            // Recurse into body for reads of outer locals
            let mut declared = Vec::new();
            let mut scope = 0;
            walk_block_locals(anon.body().block(), &mut declared, reads, &mut scope);
        }
        Expression::InterpolatedString(s) => {
            for e in s.expressions() {
                walk_expr_reads(e, reads);
            }
        }
        _ => {}
    }
}

fn walk_var_reads(var: &Var, reads: &mut HashSet<String>) {
    match var {
        Var::Name(tok) => {
            reads.insert(tok.token().to_string());
        }
        Var::Expression(ve) => {
            if let Prefix::Name(tok) = ve.prefix() {
                reads.insert(tok.token().to_string());
            } else if let Prefix::Expression(e) = ve.prefix() {
                walk_expr_reads(e, reads);
            }
            for s in ve.suffixes() {
                match s {
                    Suffix::Call(Call::AnonymousCall(args)) => walk_args_reads(args, reads),
                    Suffix::Call(Call::MethodCall(m)) => walk_args_reads(m.args(), reads),
                    Suffix::Index(full_moon::ast::Index::Brackets { expression, .. }) => {
                        walk_expr_reads(expression, reads)
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn walk_call_reads(call: &full_moon::ast::FunctionCall, reads: &mut HashSet<String>) {
    if let Prefix::Name(tok) = call.prefix() {
        reads.insert(tok.token().to_string());
    } else if let Prefix::Expression(e) = call.prefix() {
        walk_expr_reads(e, reads);
    }
    for s in call.suffixes() {
        match s {
            Suffix::Call(Call::AnonymousCall(args)) => walk_args_reads(args, reads),
            Suffix::Call(Call::MethodCall(m)) => walk_args_reads(m.args(), reads),
            Suffix::Index(full_moon::ast::Index::Brackets { expression, .. }) => {
                walk_expr_reads(expression, reads)
            }
            _ => {}
        }
    }
}

fn walk_args_reads(args: &FunctionArgs, reads: &mut HashSet<String>) {
    match args {
        FunctionArgs::Parentheses { arguments, .. } => {
            for e in arguments {
                walk_expr_reads(e, reads);
            }
        }
        FunctionArgs::TableConstructor(t) => {
            walk_expr_reads(&Expression::TableConstructor(t.clone()), reads);
        }
        _ => {}
    }
}

fn collect_key_references(
    root: &Path,
    files: &[PathBuf],
    graph: &RequireGraph,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    // Map absolute-ish rel path of required module → set of keys referenced
    let mut refs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for f in &graph.files {
        refs.entry(f.clone()).or_default();
    }

    // Build specifier resolution using existing edges
    let mut from_edges: BTreeMap<(String, String), String> = BTreeMap::new();
    for e in &graph.edges {
        if let (Some(spec), Some(to)) = (&e.specifier, &e.to) {
            from_edges.insert((e.from.clone(), spec.clone()), to.clone());
        }
    }

    for file in files {
        let from_rel = display_rel(root, file);
        let source = std::fs::read_to_string(file)
            .map_err(|e| format!("read {}: {e}", file.display()))?;
        let ast = parse(&source).map_err(|e| format!("parse {}: {e:?}", file.display()))?;

        // Bindings: local name → required module rel path
        let mut bindings: HashMap<String, String> = HashMap::new();
        scan_requires_and_refs(ast.nodes(), &from_rel, &from_edges, &mut bindings, &mut refs);
        if let Some(full_moon::ast::LastStmt::Return(r)) = ast.nodes().last_stmt() {
            for expr in r.returns() {
                scan_expr_refs(expr, &from_rel, &from_edges, &bindings, &mut refs);
            }
        }
    }
    Ok(refs)
}

fn scan_requires_and_refs(
    block: &full_moon::ast::Block,
    from_rel: &str,
    edges: &BTreeMap<(String, String), String>,
    bindings: &mut HashMap<String, String>,
    refs: &mut BTreeMap<String, BTreeSet<String>>,
) {
    for stmt in block.stmts() {
        match stmt {
            Stmt::LocalAssignment(a) => {
                let names: Vec<_> = a.names().iter().map(|n| n.token().to_string()).collect();
                let exprs: Vec<_> = a.expressions().iter().collect();
                for (i, name) in names.iter().enumerate() {
                    if let Some(expr) = exprs.get(i) {
                        if let Some(mod_path) = require_target(expr, from_rel, edges) {
                            bindings.insert(name.clone(), mod_path);
                        }
                        scan_expr_refs(expr, from_rel, edges, bindings, refs);
                    }
                }
            }
            Stmt::Assignment(a) => {
                for expr in a.expressions() {
                    scan_expr_refs(expr, from_rel, edges, bindings, refs);
                }
            }
            Stmt::FunctionCall(c) => scan_call_refs(c, from_rel, edges, bindings, refs),
            Stmt::If(i) => {
                scan_expr_refs(i.condition(), from_rel, edges, bindings, refs);
                scan_requires_and_refs(i.block(), from_rel, edges, bindings, refs);
                for e in i.else_if().into_iter().flatten() {
                    scan_expr_refs(e.condition(), from_rel, edges, bindings, refs);
                    scan_requires_and_refs(e.block(), from_rel, edges, bindings, refs);
                }
                if let Some(b) = i.else_block() {
                    scan_requires_and_refs(b, from_rel, edges, bindings, refs);
                }
            }
            Stmt::While(w) => {
                scan_expr_refs(w.condition(), from_rel, edges, bindings, refs);
                scan_requires_and_refs(w.block(), from_rel, edges, bindings, refs);
            }
            Stmt::Do(d) => scan_requires_and_refs(d.block(), from_rel, edges, bindings, refs),
            Stmt::LocalFunction(f) => {
                scan_requires_and_refs(f.body().block(), from_rel, edges, bindings, refs)
            }
            Stmt::FunctionDeclaration(f) => {
                scan_requires_and_refs(f.body().block(), from_rel, edges, bindings, refs)
            }
            Stmt::NumericFor(f) => {
                scan_requires_and_refs(f.block(), from_rel, edges, bindings, refs)
            }
            Stmt::GenericFor(f) => {
                for e in f.expressions() {
                    scan_expr_refs(e, from_rel, edges, bindings, refs);
                }
                scan_requires_and_refs(f.block(), from_rel, edges, bindings, refs);
            }
            Stmt::Repeat(r) => {
                scan_requires_and_refs(r.block(), from_rel, edges, bindings, refs);
                scan_expr_refs(r.until(), from_rel, edges, bindings, refs);
            }
            _ => {}
        }
    }
}

fn require_target(
    expr: &Expression,
    from_rel: &str,
    edges: &BTreeMap<(String, String), String>,
) -> Option<String> {
    let Expression::FunctionCall(call) = expr else {
        return None;
    };
    let Prefix::Name(tok) = call.prefix() else {
        return None;
    };
    if tok.token().to_string().trim() != "require" {
        return None;
    }
    let suffixes: Vec<_> = call.suffixes().collect();
    let Suffix::Call(Call::AnonymousCall(args)) = suffixes.first()? else {
        return None;
    };
    let FunctionArgs::Parentheses { arguments, .. } = args else {
        return None;
    };
    let first = arguments.iter().next()?;
    let Expression::String(s) = first else {
        return None;
    };
    let spec = unquote_str(&s.token().to_string());
    edges
        .get(&(from_rel.to_string(), spec))
        .cloned()
}

fn scan_expr_refs(
    expr: &Expression,
    from_rel: &str,
    edges: &BTreeMap<(String, String), String>,
    bindings: &HashMap<String, String>,
    refs: &mut BTreeMap<String, BTreeSet<String>>,
) {
    match expr {
        Expression::Var(Var::Expression(ve)) => {
            // m.key or require("x").key
            if let Prefix::Name(tok) = ve.prefix() {
                let base = tok.token().to_string();
                if let Some(mod_path) = bindings.get(&base) {
                    for s in ve.suffixes() {
                        if let Suffix::Index(full_moon::ast::Index::Dot { name, .. }) = s {
                            refs.entry(mod_path.clone())
                                .or_default()
                                .insert(name.token().to_string());
                        }
                    }
                }
            } else if let Prefix::Expression(inner) = ve.prefix() {
                if let Some(mod_path) = require_target(inner, from_rel, edges) {
                    for s in ve.suffixes() {
                        if let Suffix::Index(full_moon::ast::Index::Dot { name, .. }) = s {
                            refs.entry(mod_path.clone())
                                .or_default()
                                .insert(name.token().to_string());
                        }
                    }
                }
                scan_expr_refs(inner, from_rel, edges, bindings, refs);
            }
            for s in ve.suffixes() {
                if let Suffix::Call(Call::AnonymousCall(args)) = s {
                    if let FunctionArgs::Parentheses { arguments, .. } = args {
                        for a in arguments {
                            scan_expr_refs(a, from_rel, edges, bindings, refs);
                        }
                    }
                }
            }
        }
        Expression::FunctionCall(c) => scan_call_refs(c, from_rel, edges, bindings, refs),
        Expression::BinaryOperator { lhs, rhs, .. } => {
            scan_expr_refs(lhs, from_rel, edges, bindings, refs);
            scan_expr_refs(rhs, from_rel, edges, bindings, refs);
        }
        Expression::Parentheses { expression, .. }
        | Expression::UnaryOperator { expression, .. }
        | Expression::TypeAssertion { expression, .. } => {
            scan_expr_refs(expression, from_rel, edges, bindings, refs)
        }
        Expression::TableConstructor(t) => {
            for f in t.fields() {
                match f {
                    Field::ExpressionKey { key, value, .. } => {
                        scan_expr_refs(key, from_rel, edges, bindings, refs);
                        scan_expr_refs(value, from_rel, edges, bindings, refs);
                    }
                    Field::NameKey { value, .. } => {
                        scan_expr_refs(value, from_rel, edges, bindings, refs)
                    }
                    Field::NoKey(v) => scan_expr_refs(v, from_rel, edges, bindings, refs),
                    _ => {}
                }
            }
        }
        Expression::Function(anon) => {
            scan_requires_and_refs(
                anon.body().block(),
                from_rel,
                edges,
                &mut bindings.clone(),
                refs,
            );
        }
        _ => {}
    }
}

fn scan_call_refs(
    call: &full_moon::ast::FunctionCall,
    from_rel: &str,
    edges: &BTreeMap<(String, String), String>,
    bindings: &HashMap<String, String>,
    refs: &mut BTreeMap<String, BTreeSet<String>>,
) {
    if let Prefix::Expression(e) = call.prefix() {
        scan_expr_refs(e, from_rel, edges, bindings, refs);
    } else if let Prefix::Name(tok) = call.prefix() {
        let base = tok.token().to_string();
        // m.foo(...) — method-ish via dot then call is Suffix chain
        if let Some(mod_path) = bindings.get(&base) {
            let suffixes: Vec<_> = call.suffixes().collect();
            if let Some(Suffix::Index(full_moon::ast::Index::Dot { name, .. })) = suffixes.first()
            {
                refs.entry(mod_path.clone())
                    .or_default()
                    .insert(name.token().to_string());
            }
        }
    }
    for s in call.suffixes() {
        match s {
            Suffix::Call(Call::AnonymousCall(args)) => {
                if let FunctionArgs::Parentheses { arguments, .. } = args {
                    for a in arguments {
                        scan_expr_refs(a, from_rel, edges, bindings, refs);
                    }
                }
            }
            Suffix::Call(Call::MethodCall(m)) => {
                if let FunctionArgs::Parentheses { arguments, .. } = m.args() {
                    for a in arguments {
                        scan_expr_refs(a, from_rel, edges, bindings, refs);
                    }
                }
            }
            Suffix::Index(full_moon::ast::Index::Brackets { expression, .. }) => {
                scan_expr_refs(expression, from_rel, edges, bindings, refs)
            }
            _ => {}
        }
    }
}

/// Tarjan SCC for require cycles.
pub fn find_cycles(graph: &RequireGraph) -> Vec<CycleInfo> {
    let adj = adjacency(graph);
    let mut index = 0usize;
    let mut stack = Vec::new();
    let mut on_stack = HashSet::new();
    let mut indices: HashMap<String, usize> = HashMap::new();
    let mut lowlink: HashMap<String, usize> = HashMap::new();
    let mut cycles = Vec::new();

    fn strongconnect(
        v: &str,
        adj: &BTreeMap<String, BTreeSet<String>>,
        index: &mut usize,
        stack: &mut Vec<String>,
        on_stack: &mut HashSet<String>,
        indices: &mut HashMap<String, usize>,
        lowlink: &mut HashMap<String, usize>,
        cycles: &mut Vec<CycleInfo>,
    ) {
        indices.insert(v.to_string(), *index);
        lowlink.insert(v.to_string(), *index);
        *index += 1;
        stack.push(v.to_string());
        on_stack.insert(v.to_string());

        if let Some(ns) = adj.get(v) {
            for w in ns {
                if !indices.contains_key(w) {
                    strongconnect(w, adj, index, stack, on_stack, indices, lowlink, cycles);
                    let lw = *lowlink.get(w).unwrap();
                    let lv = *lowlink.get(v).unwrap();
                    lowlink.insert(v.to_string(), lv.min(lw));
                } else if on_stack.contains(w) {
                    let iw = *indices.get(w).unwrap();
                    let lv = *lowlink.get(v).unwrap();
                    lowlink.insert(v.to_string(), lv.min(iw));
                }
            }
        }

        if lowlink.get(v) == indices.get(v) {
            let mut comp = Vec::new();
            loop {
                let w = stack.pop().unwrap();
                on_stack.remove(&w);
                comp.push(w.clone());
                if w == v {
                    break;
                }
            }
            if comp.len() > 1 {
                comp.reverse();
                // close the cycle for display
                let first = comp[0].clone();
                comp.push(first);
                cycles.push(CycleInfo { path: comp });
            } else if comp.len() == 1 {
                // self-loop
                if adj
                    .get(&comp[0])
                    .map(|s| s.contains(&comp[0]))
                    .unwrap_or(false)
                {
                    cycles.push(CycleInfo {
                        path: vec![comp[0].clone(), comp[0].clone()],
                    });
                }
            }
        }
    }

    for f in &graph.files {
        if !indices.contains_key(f) {
            strongconnect(
                f,
                &adj,
                &mut index,
                &mut stack,
                &mut on_stack,
                &mut indices,
                &mut lowlink,
                &mut cycles,
            );
        }
    }
    cycles.sort_by(|a, b| a.path.cmp(&b.path));
    cycles
}
