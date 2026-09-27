//! Configured Luau import zones; formulas/rules follow Fallow documentation.
use crate::{
    config::{path_patterns, BoundaryZone},
    dead_code::{DeadCodeFinding, DeadKind},
    graph::RequireGraph,
};

pub(crate) fn violations(
    zones: &[BoundaryZone],
    graph: &RequireGraph,
) -> Result<Vec<DeadCodeFinding>, String> {
    let mut names = std::collections::BTreeSet::new();
    for zone in zones {
        if zone.name.is_empty() || !names.insert(&zone.name) {
            return Err(format!(
                "empty or duplicate boundary zone name: {}",
                zone.name
            ));
        }
    }
    for zone in zones {
        for allowed in &zone.allow_imports_from {
            if allowed != "*" && !names.contains(allowed) {
                return Err(format!(
                    "boundary {} allows unknown zone {allowed}",
                    zone.name
                ));
            }
        }
    }
    let patterns = zones
        .iter()
        .map(|z| path_patterns(&z.paths))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = Vec::new();
    for edge in &graph.edges {
        let Some(to) = &edge.to else { continue };
        for (source, matcher) in zones.iter().zip(&patterns) {
            if !matcher.is_match(&edge.from) {
                continue;
            }
            for (target, target_matcher) in zones.iter().zip(&patterns) {
                if !target_matcher.is_match(to)
                    || source.name == target.name
                    || source
                        .allow_imports_from
                        .iter()
                        .any(|n| n == "*" || n == &target.name)
                {
                    continue;
                }
                result.push(DeadCodeFinding {
                    path: edge.from.clone(),
                    line: edge.line,
                    kind: DeadKind::BoundaryViolation,
                    name: to.clone(),
                    message: format!(
                        "zone {} cannot import {} from zone {}",
                        source.name, to, target.name
                    ),
                });
            }
        }
    }
    Ok(result)
}
