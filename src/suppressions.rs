//! `suppressions` — inventory fallow-luau-ignore markers.

use std::path::Path;

use serde::Serialize;

use crate::discover::discover_configured_files;
use crate::graph::display_rel;
use full_moon::{node::Node, tokenizer::TokenType};

pub(crate) fn source_markers(path: &str, source: &str) -> Result<Vec<Suppression>, String> {
    let ast = full_moon::parse(source).map_err(|e| format!("parse {path}: {e:?}"))?;
    let mut markers = Vec::new();
    for reference in ast.nodes().tokens().chain(std::iter::once(ast.eof())) {
        for token in reference
            .leading_trivia()
            .chain(reference.trailing_trivia())
        {
            let TokenType::SingleLineComment { comment } = token.token_type() else {
                continue;
            };
            let text = comment.trim();
            let Some(rest) = text
                .strip_prefix("fallow-luau-ignore")
                .or_else(|| text.strip_prefix("fallow-ignore"))
            else {
                continue;
            };
            let (level, rest) = if let Some(rest) = rest.strip_prefix("-file") {
                ("file", rest)
            } else if let Some(rest) = rest.strip_prefix("-next-line") {
                ("next-line", rest)
            } else if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                ("next-line", rest)
            } else {
                continue;
            };
            let (kinds, reason) = if let Some(reason) = rest.trim().strip_prefix("-- ") {
                ("", reason)
            } else {
                rest.trim().split_once(" -- ").unwrap_or((rest.trim(), ""))
            };
            markers.push(Suppression {
                path: path.into(),
                line: token.start_position().line(),
                kind: if kinds.is_empty() {
                    "all".into()
                } else {
                    kinds.into()
                },
                level: level.into(),
                reason: (!reason.is_empty()).then(|| reason.into()),
                text: format!("-- {text}"),
            });
        }
    }
    Ok(markers)
}

pub(crate) fn suppresses(markers: &[Suppression], path: &str, line: usize, kind: &str) -> bool {
    let kind = kind.trim_start_matches("fallow-luau/").replace('_', "-");
    markers.iter().any(|m| {
        m.path == path
            && (m.level == "file" || m.line + 1 == line)
            && m.kind
                .split(|c: char| c == ',' || c.is_whitespace())
                .any(|k| {
                    let k = k.trim_start_matches("fallow-luau/").replace('_', "-");
                    k == "all"
                        || k == kind
                        || ((k == "complexity" || k == "high-complexity")
                            && kind.contains("complexity"))
                        || (k == "coverage-gaps" && kind == "high-crap")
                        || (matches!(k.as_str(), "duplication" | "clone" | "code-duplication")
                            && kind == "duplicates")
                })
    })
}

pub(crate) fn load_markers(
    root: &Path,
    files: &[std::path::PathBuf],
) -> Result<Vec<Suppression>, String> {
    let mut result = Vec::new();
    for file in files {
        let source =
            std::fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
        result.extend(source_markers(&display_rel(root, file), &source)?);
    }
    Ok(result)
}

#[derive(Debug, Clone, Serialize)]
pub struct Suppression {
    pub path: String,
    pub line: usize,
    pub kind: String,
    pub level: String,
    pub reason: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuppressionsReport {
    pub schema_version: u32,
    pub root: String,
    pub suppressions: Vec<Suppression>,
    pub totals: SuppressionsTotals,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuppressionsTotals {
    pub files_with_suppressions: usize,
    pub markers: usize,
}

pub fn collect_suppressions(root: &Path, explain: bool) -> Result<SuppressionsReport, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    let files = discover_configured_files(&root)?;
    let mut suppressions = Vec::new();
    let mut files_with = 0usize;

    for file in &files {
        let rel = display_rel(&root, file);
        let source =
            std::fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
        let markers = source_markers(&rel, &source)?;
        if !markers.is_empty() {
            files_with += 1;
        }
        suppressions.extend(markers);
    }

    Ok(SuppressionsReport {
        schema_version: 1,
        root: root.display().to_string(),
        totals: SuppressionsTotals {
            files_with_suppressions: files_with,
            markers: suppressions.len(),
        },
        suppressions,
        _meta: if explain {
            Some(serde_json::json!({
                "docs": "https://docs.fallow.tools/cli/suppressions",
                "markers": ["-- fallow-luau-ignore-next-line <kind>", "-- fallow-luau-ignore-file <kind>"]
            }))
        } else {
            None
        },
    })
}
