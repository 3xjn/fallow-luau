//! Health scores, hotspots, and refactoring targets.
//!
//! Formulas: https://docs.fallow.tools/explanations/health

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::complexity::{profiles_from_functions, FunctionMetrics, RiskProfile};
use crate::discover::is_test_path;
use crate::graph::{fan_counts, RequireGraph};
use crate::meta::health_meta;

const HALF_LIFE_DAYS: f64 = 90.0;
const LN2: f64 = std::f64::consts::LN_2;

#[derive(Debug, Clone)]
pub struct HealthOptions {
    pub max_cyclomatic: u32,
    pub max_cognitive: u32,
    pub max_unit_size: usize,
    pub explain: bool,
    pub hotspots: bool,
    pub targets: bool,
    pub file_scores: bool,
    pub complexity: bool,
    pub since_days: f64,
    pub min_commits: usize,
    /// Optional injected churn (path → commit unix timestamps) for tests.
    pub churn_override: Option<BTreeMap<String, Vec<i64>>>,
    pub now_unix: Option<i64>,
    /// When set (from dead-code), overrides per-file dead_code_ratio in MI.
    pub dead_ratio_override: Option<BTreeMap<String, f64>>,
}

impl Default for HealthOptions {
    fn default() -> Self {
        Self {
            max_cyclomatic: 20,
            max_cognitive: 15,
            max_unit_size: 60,
            explain: false,
            hotspots: true,
            targets: true,
            file_scores: true,
            complexity: true,
            since_days: 180.0,
            min_commits: 1,
            churn_override: None,
            now_unix: None,
            dead_ratio_override: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UnitProfiles {
    pub unit_size_profile: RiskProfile,
    pub unit_interfacing_profile: RiskProfile,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileScore {
    pub path: String,
    pub maintainability_index: f64,
    pub complexity_density: f64,
    pub dead_code_ratio: f64,
    pub fan_in: usize,
    pub fan_out: usize,
    pub lines: usize,
    pub function_count: usize,
    pub total_cyclomatic: u32,
    pub total_cognitive: u32,
    pub crap_max: f64,
    pub crap_above_threshold: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChurnTrend {
    Accelerating,
    Stable,
    Cooling,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hotspot {
    pub path: String,
    pub score: f64,
    pub commits: usize,
    pub weighted_commits: f64,
    pub complexity_density: f64,
    pub fan_in: usize,
    pub trend: ChurnTrend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetCategory {
    UrgentChurnComplexity,
    CircularDep,
    HighImpact,
    RemoveDeadCode,
    Complexity,
    Coupling,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefactorTarget {
    pub path: String,
    pub priority: f64,
    pub recommendation: String,
    pub category: TargetCategory,
    pub effort: Effort,
    pub factors: Vec<TargetFactor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetFactor {
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComplexityFinding {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub lines: usize,
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub rule: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LargeFunction {
    pub path: String,
    pub name: String,
    pub line: usize,
    pub lines: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub schema_version: u32,
    pub root: String,
    pub files_analyzed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub findings: Option<Vec<ComplexityFinding>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_scores: Option<Vec<FileScore>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotspots: Option<Vec<Hotspot>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<RefactorTarget>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_functions: Option<Vec<LargeFunction>>,
    pub vital_signs: UnitProfiles,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

pub struct FileAnalysis {
    pub path: String,
    pub abs: PathBuf,
    pub lines: usize,
    pub functions: Vec<FunctionMetrics>,
    pub is_test: bool,
}

/// Maintainability Index (Fallow simplified).
///
/// `MI = 100 - (density × 30) - (dead_ratio × 20) - min(ln(fan_out+1)×4, 15)`
/// clamped to \[0, 100].
pub fn maintainability_index(density: f64, dead_ratio: f64, fan_out: usize) -> f64 {
    let fan_out_penalty = ((fan_out as f64 + 1.0).ln() * 4.0).min(15.0);
    let mi = 100.0 - (density * 30.0) - (dead_ratio * 20.0) - fan_out_penalty;
    mi.clamp(0.0, 100.0)
}

/// CRAP score: `CC² × (1 − cov/100)³ + CC`.
pub fn crap_score(cc: u32, coverage_pct: f64) -> f64 {
    let cc = cc as f64;
    let uncovered = 1.0 - (coverage_pct / 100.0);
    cc * cc * uncovered.powi(3) + cc
}

/// Recency weight with 90-day half-life: `2^(-age_days / 90)`.
pub fn commit_weight(age_days: f64) -> f64 {
    (-age_days / HALF_LIFE_DAYS * LN2).exp()
}

pub fn analyze_health(
    root: &Path,
    files: &[FileAnalysis],
    graph: &RequireGraph,
    opts: &HealthOptions,
) -> HealthReport {
    let fans = fan_counts(graph);
    let test_reach = static_coverage_estimates(graph, files);

    let mut all_functions = Vec::new();
    let mut file_scores = Vec::new();
    let mut findings = Vec::new();
    let mut large_functions = Vec::new();

    for file in files {
        for f in &file.functions {
            all_functions.push((file.path.clone(), f.clone()));
            if f.cyclomatic >= opts.max_cyclomatic || f.cognitive >= opts.max_cognitive {
                let rule = if f.cyclomatic >= opts.max_cyclomatic && f.cognitive >= opts.max_cognitive
                {
                    "fallow-luau/high-complexity"
                } else if f.cyclomatic >= opts.max_cyclomatic {
                    "fallow-luau/high-cyclomatic-complexity"
                } else {
                    "fallow-luau/high-cognitive-complexity"
                };
                findings.push(ComplexityFinding {
                    path: file.path.clone(),
                    name: f.name.clone(),
                    line: f.line,
                    lines: f.lines,
                    cyclomatic: f.cyclomatic,
                    cognitive: f.cognitive,
                    rule: rule.into(),
                });
            }
            if f.lines > opts.max_unit_size {
                large_functions.push(LargeFunction {
                    path: file.path.clone(),
                    name: f.name.clone(),
                    line: f.line,
                    lines: f.lines,
                });
            }
        }

        if file.functions.is_empty() {
            continue; // exclude zero-function / barrel-like files from MI
        }

        let total_cc: u32 = file.functions.iter().map(|f| f.cyclomatic).sum();
        let total_cog: u32 = file.functions.iter().map(|f| f.cognitive).sum();
        let lines = file.lines.max(1);
        let density = total_cc as f64 / lines as f64;
        let (fan_in, fan_out) = fans.get(&file.path).copied().unwrap_or((0, 0));
        let dead_ratio = opts
            .dead_ratio_override
            .as_ref()
            .and_then(|m| m.get(&file.path).copied())
            .unwrap_or(0.0);
        let mi = maintainability_index(density, dead_ratio, fan_out);

        let cov = test_reach.get(&file.path).copied().unwrap_or(0.0);
        let mut crap_max = 0.0;
        let mut crap_above = 0usize;
        for f in &file.functions {
            let score = crap_score(f.cyclomatic, cov);
            if score > crap_max {
                crap_max = score;
            }
            if score >= 30.0 {
                crap_above += 1;
            }
        }

        file_scores.push(FileScore {
            path: file.path.clone(),
            maintainability_index: round2(mi),
            complexity_density: round4(density),
            dead_code_ratio: dead_ratio,
            fan_in,
            fan_out,
            lines: file.lines,
            function_count: file.functions.len(),
            total_cyclomatic: total_cc,
            total_cognitive: total_cog,
            crap_max: round2(crap_max),
            crap_above_threshold: crap_above,
        });
    }

    // Sort file scores by triage concern: larger of low-MI concern and CRAP risk.
    file_scores.sort_by(|a, b| {
        let concern = |s: &FileScore| {
            let mi_concern = 100.0 - s.maintainability_index;
            let risk = s.crap_max;
            mi_concern.max(risk)
        };
        concern(b)
            .partial_cmp(&concern(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });

    findings.sort_by(|a, b| {
        b.cyclomatic
            .cmp(&a.cyclomatic)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
    });
    large_functions.sort_by(|a, b| {
        b.lines
            .cmp(&a.lines)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.line.cmp(&b.line))
    });

    let density_by_path: BTreeMap<_, _> = file_scores
        .iter()
        .map(|s| (s.path.clone(), s.complexity_density))
        .collect();

    let hotspots = if opts.hotspots {
        Some(compute_hotspots(
            root,
            &file_scores,
            &fans,
            opts,
            &density_by_path,
        ))
    } else {
        None
    };

    let targets = if opts.targets {
        Some(compute_targets(
            &file_scores,
            hotspots.as_deref().unwrap_or(&[]),
            &all_functions,
        ))
    } else {
        None
    };

    let flat: Vec<_> = all_functions.iter().map(|(_, f)| f.clone()).collect();
    let (size_p, iface_p) = profiles_from_functions(&flat);

    HealthReport {
        schema_version: 1,
        root: root.display().to_string(),
        files_analyzed: files.len(),
        findings: if opts.complexity {
            Some(findings)
        } else {
            None
        },
        file_scores: if opts.file_scores {
            Some(file_scores)
        } else {
            None
        },
        hotspots,
        targets,
        large_functions: if large_functions.is_empty() {
            None
        } else {
            Some(large_functions)
        },
        vital_signs: UnitProfiles {
            unit_size_profile: size_p,
            unit_interfacing_profile: iface_p,
        },
        _meta: if opts.explain {
            Some(health_meta())
        } else {
            None
        },
    }
}

fn static_coverage_estimates(
    graph: &RequireGraph,
    files: &[FileAnalysis],
) -> BTreeMap<String, f64> {
    // Direct test require → 85%, transitive from test → 40%, else 0%.
    let mut is_test: BTreeMap<String, bool> = BTreeMap::new();
    for f in files {
        is_test.insert(f.path.clone(), f.is_test || is_test_path(Path::new(&f.path)));
    }
    for f in &graph.files {
        is_test.entry(f.clone()).or_insert_with(|| is_test_path(Path::new(f)));
    }

    let adj = crate::graph::adjacency(graph);
    let mut direct: BTreeMap<String, bool> = BTreeMap::new();
    let mut queue = std::collections::VecDeque::new();

    for e in &graph.edges {
        if let Some(to) = &e.to {
            if *is_test.get(&e.from).unwrap_or(&false) && !*is_test.get(to).unwrap_or(&false) {
                direct.insert(to.clone(), true);
                queue.push_back(to.clone());
            }
        }
    }

    let mut transitive: BTreeMap<String, bool> = BTreeMap::new();
    let mut seen = direct.keys().cloned().collect::<std::collections::BTreeSet<_>>();
    while let Some(node) = queue.pop_front() {
        if let Some(nexts) = adj.get(&node) {
            for n in nexts {
                if *is_test.get(n).unwrap_or(&false) {
                    continue;
                }
                if seen.insert(n.clone()) {
                    if !direct.contains_key(n) {
                        transitive.insert(n.clone(), true);
                    }
                    queue.push_back(n.clone());
                }
            }
        }
    }

    let mut out = BTreeMap::new();
    for f in &graph.files {
        let cov = if *direct.get(f).unwrap_or(&false) {
            85.0
        } else if *transitive.get(f).unwrap_or(&false) {
            40.0
        } else {
            0.0
        };
        out.insert(f.clone(), cov);
    }
    out
}

fn compute_hotspots(
    root: &Path,
    scores: &[FileScore],
    fans: &BTreeMap<String, (usize, usize)>,
    opts: &HealthOptions,
    density_by_path: &BTreeMap<String, f64>,
) -> Vec<Hotspot> {
    let now = opts.now_unix.unwrap_or_else(now_unix);
    let churn = opts
        .churn_override
        .clone()
        .unwrap_or_else(|| git_churn(root, opts.since_days, now));

    let mut weighted: BTreeMap<String, (usize, f64, ChurnTrend)> = BTreeMap::new();
    let window = opts.since_days;
    for (path, stamps) in &churn {
        let mut w = 0.0;
        let mut first_half = 0.0;
        let mut second_half = 0.0;
        let mid = now as f64 - (window / 2.0) * 86400.0;
        for &ts in stamps {
            let age_days = (now - ts) as f64 / 86400.0;
            if age_days < 0.0 || age_days > window {
                continue;
            }
            let weight = commit_weight(age_days);
            w += weight;
            if (ts as f64) >= mid {
                second_half += 1.0;
            } else {
                first_half += 1.0;
            }
        }
        let trend = if first_half == 0.0 && second_half == 0.0 {
            ChurnTrend::Stable
        } else if first_half == 0.0 {
            ChurnTrend::Accelerating
        } else {
            let ratio = second_half / first_half;
            if ratio > 1.5 {
                ChurnTrend::Accelerating
            } else if ratio < 0.67 {
                ChurnTrend::Cooling
            } else {
                ChurnTrend::Stable
            }
        };
        weighted.insert(path.clone(), (stamps.len(), w, trend));
    }

    let max_w = weighted
        .values()
        .map(|(_, w, _)| *w)
        .fold(0.0_f64, f64::max)
        .max(f64::EPSILON);
    let max_d = density_by_path
        .values()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(f64::EPSILON);

    let mut hotspots = Vec::new();
    for score in scores {
        let (commits, w, trend) = weighted
            .get(&score.path)
            .copied()
            .unwrap_or((0, 0.0, ChurnTrend::Stable));
        if commits < opts.min_commits && w == 0.0 {
            continue;
        }
        let density = *density_by_path.get(&score.path).unwrap_or(&0.0);
        let norm_c = w / max_w;
        let norm_d = density / max_d;
        let hs = norm_c * norm_d * 100.0;
        let (fan_in, _) = fans.get(&score.path).copied().unwrap_or((0, 0));
        hotspots.push(Hotspot {
            path: score.path.clone(),
            score: round2(hs),
            commits,
            weighted_commits: round4(w),
            complexity_density: round4(density),
            fan_in,
            trend,
        });
    }
    hotspots.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });
    hotspots
}

fn compute_targets(
    scores: &[FileScore],
    hotspots: &[Hotspot],
    functions: &[(String, FunctionMetrics)],
) -> Vec<RefactorTarget> {
    let hotspot_map: BTreeMap<_, _> = hotspots.iter().map(|h| (h.path.clone(), h)).collect();
    let fan_ins: Vec<usize> = scores.iter().map(|s| s.fan_in).collect();
    let p25 = percentile(&fan_ins, 0.25).max(2);
    let p95 = percentile(&fan_ins, 0.95).max(5);

    let mut by_path: BTreeMap<String, Vec<&FunctionMetrics>> = BTreeMap::new();
    for (path, f) in functions {
        by_path.entry(path.clone()).or_default().push(f);
    }

    let mut targets = Vec::new();
    for score in scores {
        let hs = hotspot_map.get(&score.path).copied();
        let hotspot_score = hs.map(|h| h.score).unwrap_or(0.0);
        let trend = hs.map(|h| h.trend).unwrap_or(ChurnTrend::Stable);
        let hotspot_boost = if hotspot_score > 0.0 {
            hotspot_score / 100.0
        } else {
            0.0
        };

        let priority = (score.complexity_density.min(1.0) * 30.0)
            + (hotspot_boost * 25.0)
            + (score.dead_code_ratio * 20.0)
            + ((score.fan_in as f64 / 20.0).min(1.0) * 15.0)
            + ((score.fan_out as f64 / 30.0).min(1.0) * 10.0);

        let fns = by_path.get(&score.path).map(|v| v.as_slice()).unwrap_or(&[]);
        let top_cognitive = fns.iter().map(|f| f.cognitive).max().unwrap_or(0);
        let fn_count = score.function_count;

        let effort = if score.lines < 100 && fn_count <= 3 && score.fan_in < p25 {
            Effort::Low
        } else if score.lines >= 500
            || score.fan_in >= p95
            || (fn_count >= 15 && score.complexity_density > 0.5)
        {
            Effort::High
        } else {
            Effort::Medium
        };

        // Category rules in priority order (Fallow docs).
        let category_match = if hotspot_score >= 50.0
            && matches!(trend, ChurnTrend::Accelerating)
            && score.complexity_density > 0.5
        {
            Some((
                TargetCategory::UrgentChurnComplexity,
                "Actively-changing file with growing complexity, stabilize before adding features"
                    .into(),
                vec![TargetFactor {
                    metric: "hotspot_score".into(),
                    value: hotspot_score,
                    threshold: 50.0,
                    detail: format!("hotspot {hotspot_score:.1} with accelerating trend"),
                }],
            ))
        } else if score.complexity_density > 0.3
            && (score.fan_in >= 20 || (score.fan_in >= 10 && fn_count >= 5))
        {
            Some((
                TargetCategory::HighImpact,
                format!(
                    "Split high-impact file, {} dependents amplify every change",
                    score.fan_in
                ),
                vec![TargetFactor {
                    metric: "fan_in".into(),
                    value: score.fan_in as f64,
                    threshold: 10.0,
                    detail: format!("{} files depend on this", score.fan_in),
                }],
            ))
        } else if score.dead_code_ratio >= 0.5 {
            Some((
                TargetCategory::RemoveDeadCode,
                format!(
                    "Remove unused exports to reduce surface area ({:.0}% dead)",
                    score.dead_code_ratio * 100.0
                ),
                vec![TargetFactor {
                    metric: "dead_code_ratio".into(),
                    value: score.dead_code_ratio,
                    threshold: 0.5,
                    detail: format!("dead ratio {:.0}%", score.dead_code_ratio * 100.0),
                }],
            ))
        } else if top_cognitive >= 30 {
            let top = fns
                .iter()
                .max_by_key(|f| f.cognitive)
                .map(|f| (f.name.clone(), f.cognitive))
                .unwrap();
            Some((
                TargetCategory::Complexity,
                format!(
                    "Extract {} (cognitive: {}) into smaller functions",
                    top.0, top.1
                ),
                vec![TargetFactor {
                    metric: "cognitive".into(),
                    value: top.1 as f64,
                    threshold: 30.0,
                    detail: format!("{} cognitive {}", top.0, top.1),
                }],
            ))
        } else if score.fan_out >= 15 && score.maintainability_index < 60.0 {
            Some((
                TargetCategory::Coupling,
                "Reduce coupling: too many requires reduce testability".into(),
                vec![TargetFactor {
                    metric: "fan_out".into(),
                    value: score.fan_out as f64,
                    threshold: 15.0,
                    detail: format!("fan-out {}", score.fan_out),
                }],
            ))
        } else {
            None
        };

        if let Some((category, recommendation, factors)) = category_match {
            targets.push(RefactorTarget {
                path: score.path.clone(),
                priority: round2(priority),
                recommendation,
                category,
                effort,
                factors,
            });
        }
    }

    targets.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });
    targets
}

fn percentile(values: &[usize], p: f64) -> usize {
    if values.is_empty() {
        return 0;
    }
    let mut v = values.to_vec();
    v.sort_unstable();
    let idx = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[idx.min(v.len() - 1)]
}

fn git_churn(root: &Path, since_days: f64, now: i64) -> BTreeMap<String, Vec<i64>> {
    let since = format!("{} days ago", since_days.ceil() as i64);
    let output = Command::new("git")
        .args([
            "-C",
            &root.display().to_string(),
            "log",
            "--since",
            &since,
            "--pretty=format:%ct",
            "--name-only",
            "--diff-filter=ACMR",
            "--",
            "*.lua",
            "*.luau",
        ])
        .output();

    let Ok(output) = output else {
        return BTreeMap::new();
    };
    if !output.status.success() {
        return BTreeMap::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut map: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    let mut current_ts: Option<i64> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(ts) = line.parse::<i64>() {
            current_ts = Some(ts);
            continue;
        }
        if let Some(ts) = current_ts {
            if ts > now {
                continue;
            }
            let path = line.replace('\\', "/");
            if path.ends_with(".lua") || path.ends_with(".luau") {
                map.entry(path).or_default().push(ts);
            }
        }
    }
    map
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

fn round4(x: f64) -> f64 {
    (x * 10000.0).round() / 10000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mi_formula() {
        // density 0.75, dead 1.0, fan_out 0 → penalty 0
        // 100 - 22.5 - 20 - 0 = 57.5
        let mi = maintainability_index(0.75, 1.0, 0);
        assert!((mi - 57.5).abs() < 0.01, "mi={mi}");
    }

    #[test]
    fn half_life_weights() {
        assert!((commit_weight(0.0) - 1.0).abs() < 1e-9);
        assert!((commit_weight(90.0) - 0.5).abs() < 1e-9);
        assert!((commit_weight(180.0) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn crap_untested() {
        // CC=5, cov=0 → 25 + 5 = 30
        assert!((crap_score(5, 0.0) - 30.0).abs() < 1e-9);
        assert!((crap_score(5, 100.0) - 5.0).abs() < 1e-9);
    }
}
