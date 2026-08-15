//! `flags` — feature-flag / settings-gate detection for Luau.

use std::path::Path;

use full_moon::ast::{Call, Expression, Prefix, Stmt, Suffix};
use full_moon::node::Node;
use full_moon::parse;
use serde::Serialize;

use crate::discover::discover_files;
use crate::graph::display_rel;

#[derive(Debug, Clone, Serialize)]
pub struct FlagHit {
    pub path: String,
    pub line: usize,
    pub kind: FlagKind,
    pub name: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlagKind {
    Identifier,
    GetAttribute,
    SettingsGate,
    EnvLookup,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlagsReport {
    pub schema_version: u32,
    pub root: String,
    pub flags: Vec<FlagHit>,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

pub fn analyze_flags(root: &Path, explain: bool) -> Result<FlagsReport, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    let files = discover_files(&root);
    let mut flags = Vec::new();

    for file in &files {
        let rel = display_rel(&root, file);
        let source =
            std::fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
        // Line-based heuristics (fast, plugin-friendly)
        for (i, line) in source.lines().enumerate() {
            let lower = line.to_lowercase();
            if lower.contains("featureflag")
                || lower.contains("feature_flag")
                || lower.contains("isfeatureenabled")
                || lower.contains("isenabled(")
            {
                flags.push(FlagHit {
                    path: rel.clone(),
                    line: i + 1,
                    kind: FlagKind::Identifier,
                    name: extract_string_or_ident(line).unwrap_or_else(|| "feature".into()),
                    detail: line.trim().to_string(),
                });
            }
            if lower.contains("getattribute") {
                flags.push(FlagHit {
                    path: rel.clone(),
                    line: i + 1,
                    kind: FlagKind::GetAttribute,
                    name: extract_string_or_ident(line).unwrap_or_else(|| "attribute".into()),
                    detail: line.trim().to_string(),
                });
            }
            if lower.contains("settings.") || lower.contains("usersettings") {
                flags.push(FlagHit {
                    path: rel.clone(),
                    line: i + 1,
                    kind: FlagKind::SettingsGate,
                    name: extract_string_or_ident(line).unwrap_or_else(|| "settings".into()),
                    detail: line.trim().to_string(),
                });
            }
            if lower.contains("getenv") || lower.contains("process.env") {
                flags.push(FlagHit {
                    path: rel.clone(),
                    line: i + 1,
                    kind: FlagKind::EnvLookup,
                    name: extract_string_or_ident(line).unwrap_or_else(|| "env".into()),
                    detail: line.trim().to_string(),
                });
            }
        }

        // AST pass: FF_* / FLAG_* identifiers in if conditions
        if let Ok(ast) = parse(&source) {
            scan_block_flags(ast.nodes(), &rel, &mut flags);
        }
    }

    flags.sort_by(|a, b| (&a.path, a.line, &a.name).cmp(&(&b.path, b.line, &b.name)));
    flags.dedup_by(|a, b| a.path == b.path && a.line == b.line && a.name == b.name);

    Ok(FlagsReport {
        schema_version: 1,
        root: root.display().to_string(),
        count: flags.len(),
        flags,
        _meta: if explain {
            Some(serde_json::json!({
                "docs": "https://docs.fallow.tools/cli/flags",
                "patterns": ["FeatureFlag/IsEnabled", "GetAttribute", "settings gates", "FF_/FLAG_ idents", "getenv"]
            }))
        } else {
            None
        },
    })
}

fn extract_string_or_ident(line: &str) -> Option<String> {
    // "name" or 'name'
    for quote in ['"', '\''] {
        if let Some(start) = line.find(quote) {
            if let Some(end) = line[start + 1..].find(quote) {
                let s = &line[start + 1..start + 1 + end];
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

fn scan_block_flags(block: &full_moon::ast::Block, path: &str, out: &mut Vec<FlagHit>) {
    for stmt in block.stmts() {
        match stmt {
            Stmt::If(i) => {
                scan_expr_flag_idents(i.condition(), path, out);
                scan_block_flags(i.block(), path, out);
                for e in i.else_if().into_iter().flatten() {
                    scan_expr_flag_idents(e.condition(), path, out);
                    scan_block_flags(e.block(), path, out);
                }
                if let Some(b) = i.else_block() {
                    scan_block_flags(b, path, out);
                }
            }
            Stmt::LocalFunction(f) => scan_block_flags(f.body().block(), path, out),
            Stmt::FunctionDeclaration(f) => scan_block_flags(f.body().block(), path, out),
            Stmt::Do(d) => scan_block_flags(d.block(), path, out),
            Stmt::While(w) => {
                scan_expr_flag_idents(w.condition(), path, out);
                scan_block_flags(w.block(), path, out);
            }
            Stmt::FunctionCall(c) => scan_call_flag(c, path, out),
            _ => {}
        }
    }
}

fn scan_expr_flag_idents(expr: &Expression, path: &str, out: &mut Vec<FlagHit>) {
    match expr {
        Expression::Var(full_moon::ast::Var::Name(tok)) => {
            let name = tok.token().to_string();
            if name.starts_with("FF_") || name.starts_with("FLAG_") || name.starts_with("Feature") {
                out.push(FlagHit {
                    path: path.into(),
                    line: tok.start_position().map(|p| p.line()).unwrap_or(1),
                    kind: FlagKind::Identifier,
                    name,
                    detail: "identifier in condition".into(),
                });
            }
        }
        Expression::BinaryOperator { lhs, rhs, .. } => {
            scan_expr_flag_idents(lhs, path, out);
            scan_expr_flag_idents(rhs, path, out);
        }
        Expression::UnaryOperator { expression, .. }
        | Expression::Parentheses { expression, .. } => {
            scan_expr_flag_idents(expression, path, out)
        }
        Expression::FunctionCall(c) => scan_call_flag(c, path, out),
        _ => {}
    }
}

fn scan_call_flag(call: &full_moon::ast::FunctionCall, path: &str, out: &mut Vec<FlagHit>) {
    if let Prefix::Name(tok) = call.prefix() {
        let name = tok.token().to_string();
        if name.contains("Feature") || name.contains("Flag") || name == "GetAttribute" {
            out.push(FlagHit {
                path: path.into(),
                line: tok.start_position().map(|p| p.line()).unwrap_or(1),
                kind: if name == "GetAttribute" {
                    FlagKind::GetAttribute
                } else {
                    FlagKind::Identifier
                },
                name,
                detail: "call".into(),
            });
        }
    }
    for s in call.suffixes() {
        if let Suffix::Call(Call::AnonymousCall(args)) = s {
            if let full_moon::ast::FunctionArgs::Parentheses { arguments, .. } = args {
                for a in arguments {
                    scan_expr_flag_idents(a, path, out);
                }
            }
        }
    }
}
