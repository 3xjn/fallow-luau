//! `suppressions` — inventory fallow-luau-ignore markers.

use std::path::Path;

use serde::Serialize;

use crate::discover::discover_files;
use crate::graph::display_rel;

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
    let files = discover_files(&root);
    let mut suppressions = Vec::new();
    let mut files_with = 0usize;

    for file in &files {
        let rel = display_rel(&root, file);
        let source =
            std::fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
        let mut file_hit = false;
        for (i, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed
                .strip_prefix("-- fallow-luau-ignore")
                .or_else(|| trimmed.strip_prefix("--fallow-luau-ignore"))
                .or_else(|| trimmed.strip_prefix("-- fallow-ignore"))
            {
                file_hit = true;
                let rest = rest.trim().trim_start_matches('-').trim();
                let (level, kind_reason) = if rest.starts_with("file") {
                    ("file", rest.trim_start_matches("file").trim())
                } else if rest.starts_with("next-line") {
                    ("next-line", rest.trim_start_matches("next-line").trim())
                } else {
                    ("next-line", rest)
                };
                let mut parts = kind_reason.splitn(2, char::is_whitespace);
                let kind = parts.next().unwrap_or("").trim();
                let reason = parts.next().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                suppressions.push(Suppression {
                    path: rel.clone(),
                    line: i + 1,
                    kind: if kind.is_empty() {
                        "all".into()
                    } else {
                        kind.to_string()
                    },
                    level: level.into(),
                    reason,
                    text: trimmed.to_string(),
                });
            }
        }
        if file_hit {
            files_with += 1;
        }
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
