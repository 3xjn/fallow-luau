use serde_json::{json, Value};

/// Metric definitions embedded when `--explain` or MCP is used.
pub fn health_meta() -> Value {
    json!({
        "docs": "https://docs.fallow.tools/explanations/health",
        "metrics": {
            "cyclomatic": {
                "name": "Cyclomatic complexity",
                "description": "1 + decision points (if/elseif, loops, and/or, Luau if-expressions). Every function is a unit, including nested local function.",
                "range": "[1, ∞)",
                "interpretation": "1–10 simple; 11–20 moderate; 21–50 high; 50+ very high. Default threshold 20."
            },
            "cognitive": {
                "name": "Cognitive complexity",
                "description": "Comprehension cost with nesting penalties (SonarSource model adapted to Luau).",
                "range": "[0, ∞)",
                "interpretation": "0–7 easy; 8–15 moderate; 15+ hard. Default threshold 15."
            },
            "complexity_density": {
                "name": "Complexity density",
                "description": "Total cyclomatic complexity / lines of code for the file.",
                "range": "[0, ∞)",
                "interpretation": "<0.3 low; 0.3–1.0 moderate; >1.0 very dense."
            },
            "maintainability_index": {
                "name": "Maintainability Index",
                "description": "100 - (density×30) - (dead_ratio×20) - min(ln(fan_out+1)×4, 15), clamped to [0,100].",
                "range": "[0, 100]",
                "interpretation": "higher is better; <40 poor, 40–70 moderate, >70 good"
            },
            "unit_size": {
                "name": "SIG unit size",
                "description": "Function length in lines. Bins: 1–15 low, 16–30 medium, 31–60 high, >60 very high.",
                "range": "LOC bins",
                "interpretation": "Flag functions over 60 lines by default."
            },
            "unit_interfacing": {
                "name": "SIG unit interfacing",
                "description": "Parameter-count bins: 0–2 low, 3–4 medium, 5–6 high, 7+ very high.",
                "range": "parameter bins",
                "interpretation": "Prefer few parameters per function."
            },
            "crap": {
                "name": "CRAP",
                "description": "CC² × (1 − cov/100)³ + CC. Default coverage from test require graph (85% direct / 40% transitive / 0%).",
                "range": "[0, ∞)",
                "interpretation": "<30 acceptable; 30–99 moderate; 100+ high."
            },
            "hotspot_score": {
                "name": "Hotspot score",
                "description": "normalized_churn × normalized_density × 100. Churn uses 90-day half-life on commits.",
                "range": "[0, 100]",
                "interpretation": "higher = more risky; project-relative."
            },
            "target_priority": {
                "name": "Refactoring target priority",
                "description": "min(density,1)×30 + hotspot_boost×25 + dead_ratio×20 + min(fan_in/20,1)×15 + min(fan_out/30,1)×10",
                "range": "[0, 100]",
                "interpretation": "higher priority first."
            }
        }
    })
}

pub fn schema_meta() -> Value {
    json!({
        "docs": "https://docs.fallow.tools/cli/schema",
        "note": "Capability manifest for fallow-luau. Algorithms port Fallow published formulas to Luau.",
        "parity": "docs/parity.md"
    })
}

pub fn dead_code_meta() -> Value {
    json!({
        "docs": "https://docs.fallow.tools/explanations/dead-code",
        "parity": "docs/parity.md",
        "issue_types": {
            "unused_file": "Unreachable from entry points (init/main + tests; fan-in=0 library roots when no init).",
            "unused_export": "Returned module-table key never referenced via require binding.",
            "unused_local": "Local binding never read (nested functions included).",
            "circular_dependency": "Require-graph SCC with no depth limit."
        }
    })
}

pub fn dupes_meta() -> Value {
    json!({
        "docs": "https://docs.fallow.tools/explanations/duplication",
        "mode": "mild (whitespace-insensitive token match; strings/numbers normalized)",
        "defaults": { "min_tokens": 30, "min_lines": 5, "min_occurrences": 2 }
    })
}
