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

use full_moon::ast::{Call, Expression, Field, FunctionArgs, Prefix, Stmt, Suffix, Var};
use full_moon::node::Node;
use full_moon::parse;
use full_moon::visitors::Visitor;
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
    UnusedType,
    CircularDependency,
    BoundaryViolation,
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
    pub unused_types: Vec<DeadCodeFinding>,
    pub total_exports: usize,
    pub export_count_by_file: BTreeMap<String, usize>,
    pub exports_by_file: BTreeMap<String, Vec<String>>,
    pub cycles: Vec<CycleInfo>,
    pub boundary_violations: Vec<DeadCodeFinding>,
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
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let config = crate::config::load_config(&root)?.config;
    let mut entry_patterns = config.entry;
    entry_patterns.extend(opts.extra_entries.clone());
    let matcher = crate::config::path_patterns(&entry_patterns)?;
    let matched: Vec<_> = graph
        .files
        .iter()
        .filter(|f| matcher.is_match(f))
        .cloned()
        .collect();
    if !entry_patterns.is_empty() && matched.is_empty() {
        return Err("entry patterns did not match any analyzed files".into());
    }
    let entries = detect_entry_points(&root, files, graph, &matched);
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
        let source =
            std::fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
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
            // An entry module's public API may be used outside this project.
            if !entries.contains(path) && !refs.contains(name) && !refs.contains("*") {
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
        dead_ratio_by_file.insert(path.clone(), unused as f64 / info.exports.len() as f64);
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

    let mut unused_types = Vec::new();
    for (path, info) in &modules {
        for (name, line) in &info.type_decls {
            if !info.type_refs.contains(name) {
                unused_types.push(DeadCodeFinding {
                    path: path.clone(),
                    kind: DeadKind::UnusedType,
                    name: name.clone(),
                    line: *line,
                    message: format!("type `{name}` is never referenced"),
                });
            }
        }
    }
    unused_types.sort_by(|a, b| (&a.path, a.line, &a.name).cmp(&(&b.path, b.line, &b.name)));

    let markers = crate::suppressions::load_markers(&root, files)?;
    let suppressed = |path: &str, line: usize, kind: &str| {
        crate::suppressions::suppresses(&markers, path, line, kind)
    };
    unused_files.retain(|path| !suppressed(path, 1, "unused-file"));
    unused_exports.retain(|f| !suppressed(&f.path, f.line, "unused-export"));
    unused_locals.retain(|f| !suppressed(&f.path, f.line, "unused-local"));
    unused_types.retain(|f| !suppressed(&f.path, f.line, "unused-type"));
    for (path, ratio) in &mut dead_ratio_by_file {
        let total = modules[path].exports.len();
        *ratio = if total == 0 {
            0.0
        } else {
            unused_exports.iter().filter(|f| f.path == *path).count() as f64 / total as f64
        };
    }
    let mut cycles = find_cycles(graph);
    cycles.retain(|c| {
        !c.path
            .iter()
            .any(|p| suppressed(p, 1, "circular-dependency"))
    });
    let mut boundary_violations = crate::boundaries::violations(&config.boundaries, graph)?;
    boundary_violations.retain(|f| !suppressed(&f.path, f.line, "boundary-violation"));
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
    findings.extend(unused_types.clone());
    findings.extend(boundary_violations.clone());
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
        unused_types,
        total_exports: modules.values().map(|m| m.exports.len()).sum(),
        export_count_by_file: modules
            .iter()
            .map(|(p, m)| (p.clone(), m.exports.len()))
            .collect(),
        exports_by_file: modules
            .iter()
            .map(|(p, m)| {
                (
                    p.clone(),
                    m.exports.iter().map(|(n, _)| n.clone()).collect(),
                )
            })
            .collect(),
        cycles,
        boundary_violations,
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
        if extra.is_empty()
            && (name == "init.lua"
                || name == "init.luau"
                || name == "main.lua"
                || name == "main.luau"
                || name.starts_with("main."))
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
    type_decls: Vec<(String, usize)>,
    type_refs: std::collections::HashSet<String>,
}

#[derive(Debug, Clone)]
struct LocalBinding {
    name: String,
    line: usize,
}

fn extract_module_info(ast: &full_moon::ast::Ast) -> ModuleInfo {
    let mut exports = Vec::new();
    if let Some(last) = ast.nodes().last_stmt() {
        if let full_moon::ast::LastStmt::Return(r) = last {
            if let Some(expr) = r.returns().iter().next() {
                collect_table_keys(expr, &mut exports);
            }
        }
    }

    let unused_locals = find_unused_locals(ast);
    let (type_decls, type_refs) = collect_types(ast);
    ModuleInfo {
        exports,
        unused_locals,
        type_decls,
        type_refs,
    }
}

fn collect_types(
    ast: &full_moon::ast::Ast,
) -> (Vec<(String, usize)>, std::collections::HashSet<String>) {
    let mut decls = Vec::new();
    let mut refs = TypeReferences::default();
    refs.visit_ast(ast);
    for stmt in ast.nodes().stmts() {
        match stmt {
            Stmt::TypeDeclaration(td) => {
                let name = td.type_name().token().to_string();
                let line = td
                    .type_name()
                    .start_position()
                    .map(|p| p.line())
                    .unwrap_or(1);
                decls.push((name, line));
            }
            // Exported types can be consumed outside the analyzed project.
            Stmt::ExportedTypeDeclaration(_) => {}
            _ => {}
        }
    }
    (decls, refs.0)
}

#[derive(Default)]
struct TypeReferences(HashSet<String>);

impl Visitor for TypeReferences {
    fn visit_type_info(&mut self, info: &full_moon::ast::luau::TypeInfo) {
        use full_moon::ast::luau::TypeInfo;
        match info {
            TypeInfo::Basic(token) | TypeInfo::Generic { base: token, .. } => {
                self.0.insert(token.token().to_string());
            }
            _ => {}
        }
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
        .filter(|(name, _, _)| name != "_" && !name.starts_with('_') && !reads.contains(name))
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
    if let Some(full_moon::ast::LastStmt::Return(ret)) = block.last_stmt() {
        for expr in ret.returns() {
            walk_expr_reads(expr, reads);
        }
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
            // An indexed write reads the table and any index expression.
            for var in a.variables() {
                if matches!(var, Var::Expression(_)) {
                    walk_var_reads(var, reads);
                }
            }
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
        Stmt::CompoundAssignment(c) => {
            walk_var_reads(c.lhs(), reads);
            walk_expr_reads(c.rhs(), reads);
        }
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
    let mut refs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for reference in crate::references::collect(root, files, graph)? {
        refs.entry(reference.to)
            .or_default()
            .insert(reference.key.unwrap_or_else(|| "*".into()));
    }
    Ok(refs)
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
                // SCC order need not follow real edges. Build a closed walk
                // through its members so every displayed step is actionable.
                comp.sort();
                let members: BTreeSet<_> = comp.iter().cloned().collect();
                let mut path = vec![comp[0].clone()];
                for to in comp.iter().skip(1).chain(std::iter::once(&comp[0])) {
                    let from = path.last().unwrap().clone();
                    let mut queue = VecDeque::from([from.clone()]);
                    let mut previous = BTreeMap::from([(from.clone(), None::<String>)]);
                    while let Some(node) = queue.pop_front() {
                        if node == *to {
                            break;
                        }
                        for next in adj.get(&node).into_iter().flatten() {
                            if members.contains(next) && !previous.contains_key(next) {
                                previous.insert(next.clone(), Some(node.clone()));
                                queue.push_back(next.clone());
                            }
                        }
                    }
                    let mut segment = vec![to.clone()];
                    let mut cursor = to;
                    while let Some(Some(parent)) = previous.get(cursor) {
                        if parent == &from {
                            break;
                        }
                        segment.push(parent.clone());
                        cursor = parent;
                    }
                    segment.reverse();
                    path.extend(segment);
                }
                cycles.push(CycleInfo { path });
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
