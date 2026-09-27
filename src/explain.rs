//! `explain` — rule docs without running analysis.

use serde_json::{json, Value};

pub fn explain_rule(id: &str) -> Value {
    let id = id.trim().trim_start_matches("fallow-luau/");
    let entry = RULES.iter().find(|(k, _)| *k == id || id.ends_with(k));
    match entry {
        Some((key, rule)) => json!({
            "schema_version": 1,
            "id": key,
            "rule_id": format!("fallow-luau/{key}"),
            "name": rule.name,
            "description": rule.description,
            "action": rule.action,
            "docs": rule.docs,
            "_meta": {
                "docs": "https://docs.fallow.tools/cli/explain",
                "note": "Luau adaptations documented in docs/parity.md"
            }
        }),
        None => json!({
            "schema_version": 1,
            "error": format!("unknown issue type `{id}`"),
            "known": RULES.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        }),
    }
}

struct Rule {
    name: &'static str,
    description: &'static str,
    action: &'static str,
    docs: &'static str,
}

const RULES: &[(&str, Rule)] = &[
    ("boundary-violation", Rule {
        name: "Architecture boundary violation",
        description: "A resolved require crosses configured zones in a forbidden direction.",
        action: "Check the dependency direction; move shared behavior or explicitly allow the dependency in boundaries.",
        docs: "https://fallow.tools/docs/configuration/boundaries/",
    }),
    ("high-crap", Rule {
        name: "Untested complexity risk",
        description: "CC squared times uncovered fraction cubed plus CC reaches max_crap (default 30). Coverage is statically estimated.",
        action: "Inspect existing tests, add behavioral coverage, then simplify the risky function if justified.",
        docs: "https://fallow.tools/docs/explanations/health/#untested-complexity-risk",
    }),
    ("large-function", Rule {
        name: "Large function",
        description: "Function exceeds max_unit_size (default 60 lines). Raw size profiles still include suppressed findings.",
        action: "Inspect responsibilities and extract cohesive helpers with regression coverage when useful.",
        docs: "https://fallow.tools/docs/explanations/health/#risk-profiles",
    }),
    ("unused-file", Rule {
        name: "Unused file",
        description: "File is not reachable from any entry point via string-literal require edges.",
        action: "Verify runtime consumers and unresolved imports before deleting; configure public entry points when appropriate.",
        docs: "https://docs.fallow.tools/explanations/dead-code#unused-files",
    }),
    ("unused-export", Rule {
        name: "Unused returned key",
        description: "Key on a returned module table is never referenced by another module.",
        action: "Verify consumers, dynamic access and public API contracts before removing the key.",
        docs: "https://docs.fallow.tools/explanations/dead-code#unused-exports",
    }),
    ("unused-local", Rule {
        name: "Unused local",
        description: "Local binding is never read (checked inside nested functions too).",
        action: "Remove the binding or prefix with `_` if intentionally unused.",
        docs: "https://docs.fallow.tools/explanations/dead-code",
    }),
    ("unused-type", Rule {
        name: "Unused type",
        description: "Luau type / export type declaration never referenced in the file.",
        action: "Remove the type or start using it.",
        docs: "https://docs.fallow.tools/explanations/dead-code#unused-types",
    }),
    ("circular-dependency", Rule {
        name: "Circular require",
        description: "Require-graph cycle with no depth limit (Tarjan SCC).",
        action: "Break the cycle by extracting a shared module.",
        docs: "https://docs.fallow.tools/explanations/dead-code#circular-dependencies",
    }),
    ("high-cyclomatic-complexity", Rule {
        name: "High cyclomatic complexity",
        description: "Function has too many linearly independent paths (default threshold 20).",
        action: "Split into smaller functions.",
        docs: "https://docs.fallow.tools/explanations/health#cyclomatic-complexity",
    }),
    ("high-cognitive-complexity", Rule {
        name: "High cognitive complexity",
        description: "Function is hard to follow top-to-bottom (default threshold 15).",
        action: "Extract helpers; reduce nesting.",
        docs: "https://docs.fallow.tools/explanations/health#cognitive-complexity",
    }),
    ("high-complexity", Rule {
        name: "High complexity",
        description: "Function exceeds both cyclomatic and cognitive thresholds.",
        action: "Refactor and/or add tests.",
        docs: "https://docs.fallow.tools/explanations/health",
    }),
    ("clone", Rule {
        name: "Duplicated code",
        description: "Token/suffix-array clone group across .lua/.luau files.",
        action: "Extract a shared function or module.",
        docs: "https://docs.fallow.tools/explanations/duplication",
    }),
];

pub fn known_rules() -> Vec<&'static str> {
    RULES.iter().map(|(k, _)| *k).collect()
}
