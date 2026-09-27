//! Token / suffix-array clone detection for Luau (Fallow dupes, mild mode).
//! Docs: https://docs.fallow.tools/cli/dupes

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::graph::display_rel;
use crate::meta::dupes_meta;

#[derive(Debug, Clone, Serialize)]
pub struct CloneInstance {
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CloneGroup {
    pub fingerprint: String,
    pub token_count: usize,
    pub line_count: usize,
    pub instances: Vec<CloneInstance>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DupesReport {
    pub schema_version: u32,
    pub root: String,
    pub mode: String,
    pub clone_groups: Vec<CloneGroup>,
    pub stats: DupesStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DupesStats {
    pub clone_groups: usize,
    pub clone_instances: usize,
    pub duplicated_lines: usize,
    pub total_lines: usize,
    pub duplication_percentage: f64,
}

#[derive(Debug, Clone)]
pub struct DupesOptions {
    pub explain: bool,
    pub min_tokens: usize,
    pub min_lines: usize,
    pub min_occurrences: usize,
}

impl Default for DupesOptions {
    fn default() -> Self {
        Self {
            explain: false,
            min_tokens: 30,
            min_lines: 5,
            min_occurrences: 2,
        }
    }
}

#[derive(Clone)]
struct Tok {
    text: String,
    line: usize,
    end_line: usize,
    file_idx: usize,
}

pub fn analyze_dupes(
    root: &Path,
    files: &[PathBuf],
    opts: &DupesOptions,
) -> Result<DupesReport, String> {
    if opts.min_tokens == 0 || opts.min_lines == 0 || opts.min_occurrences < 2 {
        return Err(
            "duplicates require positive token/line minima and at least two occurrences".into(),
        );
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let markers = crate::suppressions::load_markers(&root, files)?;
    let mut corpus: Vec<Tok> = Vec::new();
    let mut file_lines: Vec<usize> = Vec::new();
    let mut rels = Vec::new();

    for (fi, file) in files.iter().enumerate() {
        let rel = display_rel(&root, file);
        let source =
            std::fs::read_to_string(file).map_err(|e| format!("read {}: {e}", file.display()))?;
        let lines = source.lines().count();
        file_lines.push(lines);
        rels.push(rel);
        for tok in tokenize(&source)? {
            corpus.push(Tok {
                text: tok.0,
                line: tok.1,
                end_line: tok.2,
                file_idx: fi,
            });
        }
        // Sentinel to stop cross-file matches spanning the boundary without a gap.
        corpus.push(Tok {
            text: format!("\0EOF:{fi}"),
            line: lines.max(1),
            end_line: lines.max(1),
            file_idx: fi,
        });
    }

    let n = corpus.len();
    let total_lines: usize = file_lines.iter().sum();
    if n / 2 < opts.min_tokens {
        return Ok(empty_report(&root, opts, total_lines));
    }

    // Build suffix array on token texts.
    let mut sa: Vec<usize> = (0..n).collect();
    sa.sort_by(|&a, &b| {
        let ta = &corpus[a..];
        let tb = &corpus[b..];
        for i in 0..ta.len().min(tb.len()) {
            match ta[i].text.cmp(&tb[i].text) {
                std::cmp::Ordering::Equal => {}
                o => return o,
            }
            // Don't cross file sentinel in LCP interest — still lexicographic ok
        }
        ta.len().cmp(&tb.len())
    });

    // LCP between adjacent suffixes
    let mut groups: BTreeMap<String, CloneGroup> = BTreeMap::new();
    for i in 0..sa.len().saturating_sub(1) {
        let a = sa[i];
        let b = sa[i + 1];
        let lcp = longest_common_prefix(&corpus, a, b, opts.min_tokens);
        if lcp < opts.min_tokens {
            continue;
        }
        // Reject if either span crosses a sentinel
        if span_has_sentinel(&corpus, a, lcp) || span_has_sentinel(&corpus, b, lcp) {
            continue;
        }
        if corpus[a].file_idx == corpus[b].file_idx {
            // same-file: require non-overlapping
            let a_end = corpus[a + lcp - 1].line;
            let b_start = corpus[b].line;
            if a_end >= b_start && corpus[b + lcp - 1].line >= corpus[a].line {
                // overlap — skip unless well separated
                if (corpus[a].line as isize - corpus[b].line as isize).unsigned_abs()
                    < opts.min_lines
                {
                    continue;
                }
            }
        }
        let line_a0 = corpus[a].line;
        let line_a1 = corpus[a + lcp - 1].end_line;
        let line_b0 = corpus[b].line;
        let line_b1 = corpus[b + lcp - 1].end_line;
        let lines_a = line_a1.saturating_sub(line_a0).saturating_add(1);
        let lines_b = line_b1.saturating_sub(line_b0).saturating_add(1);
        if lines_a < opts.min_lines || lines_b < opts.min_lines {
            continue;
        }
        let key_tokens: Vec<&str> = (0..lcp).map(|k| corpus[a + k].text.as_str()).collect();
        let fingerprint = format!("dup:{:08x}", fnv1a(&key_tokens.join("\0")));
        let entry = groups.entry(fingerprint.clone()).or_insert(CloneGroup {
            fingerprint: fingerprint.clone(),
            token_count: lcp,
            line_count: lines_a.max(lines_b),
            instances: Vec::new(),
        });
        if entry.token_count < lcp {
            entry.token_count = lcp;
            entry.line_count = lines_a.max(lines_b);
        }
        push_instance(
            entry,
            CloneInstance {
                path: rels[corpus[a].file_idx].clone(),
                start_line: line_a0,
                end_line: line_a1,
            },
        );
        push_instance(
            entry,
            CloneInstance {
                path: rels[corpus[b].file_idx].clone(),
                start_line: line_b0,
                end_line: line_b1,
            },
        );
    }

    let mut clone_groups: Vec<CloneGroup> = groups
        .into_values()
        .map(|mut group| {
            group.instances.retain(|i| {
                !(i.start_line..=i.end_line).any(|line| {
                    crate::suppressions::suppresses(&markers, &i.path, line, "duplicates")
                })
            });
            group
        })
        .filter(|g| g.instances.len() >= opts.min_occurrences)
        .collect();
    clone_groups.sort_by(|a, b| {
        (b.token_count * b.instances.len())
            .cmp(&(a.token_count * a.instances.len()))
            .then_with(|| a.fingerprint.cmp(&b.fingerprint))
    });

    let mut covered_lines = std::collections::BTreeSet::new();
    let mut instance_count = 0usize;
    for g in &clone_groups {
        instance_count += g.instances.len();
        for inst in &g.instances {
            for line in inst.start_line..=inst.end_line {
                covered_lines.insert((inst.path.clone(), line));
            }
        }
    }
    let duplicated_lines = covered_lines.len();
    let pct = if total_lines == 0 {
        0.0
    } else {
        (duplicated_lines as f64) * 100.0 / (total_lines as f64)
    };

    Ok(DupesReport {
        schema_version: 1,
        root: root.display().to_string(),
        mode: "mild".into(),
        clone_groups,
        stats: DupesStats {
            clone_groups: 0, // set below
            clone_instances: instance_count,
            duplicated_lines,
            total_lines,
            duplication_percentage: (pct * 100.0).round() / 100.0,
        },
        _meta: if opts.explain {
            Some(dupes_meta())
        } else {
            None
        },
    }
    .with_group_count())
}

impl DupesReport {
    pub(crate) fn retain_groups(&mut self, keep: impl Fn(&CloneGroup) -> bool) {
        self.clone_groups.retain(keep);
        let mut lines = std::collections::BTreeSet::new();
        self.stats.clone_instances = 0;
        for group in &self.clone_groups {
            self.stats.clone_instances += group.instances.len();
            for inst in &group.instances {
                lines.extend(
                    (inst.start_line..=inst.end_line).map(|line| (inst.path.clone(), line)),
                );
            }
        }
        self.stats.clone_groups = self.clone_groups.len();
        self.stats.duplicated_lines = lines.len();
        self.stats.duplication_percentage = if self.stats.total_lines == 0 {
            0.0
        } else {
            (lines.len() as f64 * 10000.0 / self.stats.total_lines as f64).round() / 100.0
        };
    }
}

trait WithGroupCount {
    fn with_group_count(self) -> Self;
}

impl WithGroupCount for DupesReport {
    fn with_group_count(mut self) -> Self {
        self.stats.clone_groups = self.clone_groups.len();
        self
    }
}

fn empty_report(root: &Path, opts: &DupesOptions, total_lines: usize) -> DupesReport {
    DupesReport {
        schema_version: 1,
        root: root.display().to_string(),
        mode: "mild".into(),
        clone_groups: Vec::new(),
        stats: DupesStats {
            clone_groups: 0,
            clone_instances: 0,
            duplicated_lines: 0,
            total_lines,
            duplication_percentage: 0.0,
        },
        _meta: if opts.explain {
            Some(dupes_meta())
        } else {
            None
        },
    }
}

fn push_instance(group: &mut CloneGroup, inst: CloneInstance) {
    if !group.instances.iter().any(|i| {
        i.path == inst.path && i.start_line == inst.start_line && i.end_line == inst.end_line
    }) {
        group.instances.push(inst);
    }
}

fn longest_common_prefix(corpus: &[Tok], a: usize, b: usize, min: usize) -> usize {
    let mut i = 0;
    while a + i < corpus.len() && b + i < corpus.len() {
        if corpus[a + i].text != corpus[b + i].text {
            break;
        }
        if corpus[a + i].text.starts_with("\0EOF:") {
            break;
        }
        i += 1;
    }
    if i < min {
        0
    } else {
        i
    }
}

fn span_has_sentinel(corpus: &[Tok], start: usize, len: usize) -> bool {
    corpus[start..start + len]
        .iter()
        .any(|t| t.text.starts_with("\0EOF:"))
}

fn tokenize(source: &str) -> Result<Vec<(String, usize, usize)>, String> {
    use full_moon::{node::Node, tokenizer::TokenType};
    let ast = full_moon::parse(source).map_err(|e| format!("parse duplicate corpus: {e:?}"))?;
    Ok(ast
        .nodes()
        .tokens()
        .map(|reference| {
            let token = reference.token();
            let text = match token.token_type() {
                TokenType::StringLiteral { .. } => "\0STRING".into(),
                TokenType::Number { .. } => "\0NUMBER".into(),
                _ => token.to_string(),
            };
            (
                text,
                token.start_position().line(),
                token.end_position().line(),
            )
        })
        .collect())
}
fn fnv1a(s: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}
