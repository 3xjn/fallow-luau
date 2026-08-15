//! `watch` — re-run analysis when Luau files change (mtime poll).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::audit::{analyze_audit, AuditOptions};
use crate::discover::discover_files;

pub fn watch_loop(
    root: &Path,
    interval_ms: u64,
    changed_since: Option<String>,
    explain: bool,
) -> Result<(), String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    eprintln!(
        "fallow-luau watch: polling {} every {}ms (Ctrl-C to stop)",
        root.display(),
        interval_ms
    );
    let mut last: BTreeMap<PathBuf, SystemTime> = snapshot(&root);
    // Initial run
    run_once(&root, &changed_since, explain)?;

    loop {
        std::thread::sleep(Duration::from_millis(interval_ms.max(200)));
        let now = snapshot(&root);
        if now != last {
            eprintln!("change detected — re-running audit");
            run_once(&root, &changed_since, explain)?;
            last = now;
        }
    }
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, SystemTime> {
    let mut map = BTreeMap::new();
    for f in discover_files(root) {
        if let Ok(meta) = std::fs::metadata(&f) {
            if let Ok(mtime) = meta.modified() {
                map.insert(f, mtime);
            }
        }
    }
    map
}

fn run_once(root: &Path, changed_since: &Option<String>, explain: bool) -> Result<(), String> {
    let report = analyze_audit(
        root,
        &AuditOptions {
            explain,
            changed_since: changed_since.clone(),
            ..AuditOptions::default()
        },
    )?;
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    eprintln!("verdict: {:?}", report.verdict);
    Ok(())
}
