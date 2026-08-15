//! `report` — re-render a saved JSON results file without re-running analysis.

use std::path::Path;

use serde_json::Value;

#[derive(Debug, Clone, Copy)]
pub enum ReportFormat {
    Json,
    Compact,
    Markdown,
}

pub fn render_saved_report(path: &Path, format: ReportFormat) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
    match format {
        ReportFormat::Json => Ok(serde_json::to_string_pretty(&value).unwrap()),
        ReportFormat::Compact => Ok(serde_json::to_string(&value).unwrap()),
        ReportFormat::Markdown => Ok(json_to_markdown(&value)),
    }
}

fn json_to_markdown(v: &Value) -> String {
    let mut out = String::from("# fallow-luau report\n\n");
    if let Some(verdict) = v.get("verdict").and_then(|x| x.as_str()) {
        out.push_str(&format!("**Verdict:** `{verdict}`\n\n"));
    }
    if let Some(score) = v.pointer("/health_score/score").and_then(|x| x.as_f64()) {
        let grade = v
            .pointer("/health_score/grade")
            .and_then(|x| x.as_str())
            .unwrap_or("?");
        out.push_str(&format!("**Health score:** {score:.1} ({grade})\n\n"));
    }
    if let Some(findings) = v
        .get("findings")
        .or_else(|| v.pointer("/health/findings"))
        .and_then(|x| x.as_array())
    {
        out.push_str("## Findings\n\n");
        for f in findings {
            let path = f.get("path").and_then(|x| x.as_str()).unwrap_or("?");
            let name = f.get("name").and_then(|x| x.as_str()).unwrap_or("");
            let line = f.get("line").and_then(|x| x.as_u64()).unwrap_or(0);
            out.push_str(&format!("- `{path}:{line}` {name}\n"));
        }
        out.push('\n');
    }
    if let Some(unused) = v
        .pointer("/unused_files")
        .or_else(|| v.pointer("/dead_code/unused_files"))
        .and_then(|x| x.as_array())
    {
        out.push_str("## Unused files\n\n");
        for u in unused {
            if let Some(s) = u.as_str() {
                out.push_str(&format!("- `{s}`\n"));
            }
        }
        out.push('\n');
    }
    if let Some(groups) = v
        .pointer("/clone_groups")
        .or_else(|| v.pointer("/dupes/clone_groups"))
        .and_then(|x| x.as_array())
    {
        out.push_str(&format!("## Clones ({})\n\n", groups.len()));
    }
    out.push_str("_Re-rendered by `fallow-luau report` without re-analysis._\n");
    out
}
