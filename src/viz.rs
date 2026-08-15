//! `viz` — self-contained HTML treemap + require graph.

use std::path::Path;

use crate::complexity::analyze_functions;
use crate::discover::{discover_files, is_test_path};
use crate::graph::{build_require_graph, display_rel, fan_counts};
use crate::health::{analyze_health, FileAnalysis, HealthOptions};
use crate::dead_code::{analyze_dead_code, DeadCodeOptions};
use full_moon::parse;

pub fn render_viz_html(root: &Path) -> Result<String, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("canonicalize {}: {e}", root.display()))?;
    let files = discover_files(&root);
    let graph = build_require_graph(&root, &files)?;
    let dead = analyze_dead_code(
        &root,
        &files,
        &graph,
        &DeadCodeOptions {
            explain: false,
            ..Default::default()
        },
    )?;

    let mut analyses = Vec::new();
    for file in &files {
        let src = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let path = display_rel(&root, file);
        let ast = parse(&src).map_err(|e| format!("{e:?}"))?;
        analyses.push(FileAnalysis {
            is_test: is_test_path(Path::new(&path)),
            path,
            abs: file.clone(),
            lines: src.lines().count(),
            functions: analyze_functions(&ast),
        });
    }
    let health = analyze_health(
        &root,
        &analyses,
        &graph,
        &HealthOptions {
            explain: false,
            hotspots: false,
            targets: false,
            dead_ratio_override: Some(dead.dead_ratio_by_file.clone()),
            ..HealthOptions::default()
        },
    );
    let fans = fan_counts(&graph);
    let scores = health.file_scores.unwrap_or_default();

    let mut nodes = Vec::new();
    for s in &scores {
        let (fi, fo) = fans.get(&s.path).copied().unwrap_or((0, 0));
        nodes.push(serde_json::json!({
            "path": s.path,
            "lines": s.lines,
            "mi": s.maintainability_index,
            "density": s.complexity_density,
            "fan_in": fi,
            "fan_out": fo,
            "dead_ratio": s.dead_code_ratio
        }));
    }
    // Include files with zero functions
    for f in &graph.files {
        if !nodes.iter().any(|n| n["path"] == *f) {
            let (fi, fo) = fans.get(f).copied().unwrap_or((0, 0));
            let lines = analyses
                .iter()
                .find(|a| a.path == *f)
                .map(|a| a.lines)
                .unwrap_or(1);
            nodes.push(serde_json::json!({
                "path": f,
                "lines": lines,
                "mi": null,
                "density": 0.0,
                "fan_in": fi,
                "fan_out": fo,
                "dead_ratio": dead.dead_ratio_by_file.get(f).copied().unwrap_or(0.0)
            }));
        }
    }

    let edges: Vec<_> = graph
        .edges
        .iter()
        .filter_map(|e| {
            e.to.as_ref().map(|to| {
                serde_json::json!({ "from": e.from, "to": to, "line": e.line })
            })
        })
        .collect();

    let data = serde_json::json!({
        "root": root.display().to_string(),
        "nodes": nodes,
        "edges": edges,
        "unused_files": dead.unused_files,
        "cycles": dead.cycles
    });

    let data_json = serde_json::to_string(&data).unwrap();
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>fallow-luau viz</title>
<style>
  :root {{ --bg:#0f1419; --fg:#e7ecf1; --muted:#8b9aab; --accent:#3d9a78; --warn:#c9843e; --bad:#c44b4b; }}
  * {{ box-sizing: border-box; }}
  body {{ margin:0; font:14px/1.4 ui-sans-serif, system-ui, sans-serif; background:var(--bg); color:var(--fg); }}
  header {{ padding:1rem 1.25rem; border-bottom:1px solid #243041; }}
  h1 {{ margin:0; font-size:1.1rem; font-weight:600; letter-spacing:.02em; }}
  header p {{ margin:.35rem 0 0; color:var(--muted); font-size:.85rem; }}
  main {{ display:grid; grid-template-columns:1fr 1fr; gap:0; min-height:calc(100vh - 72px); }}
  section {{ padding:1rem; border-right:1px solid #243041; }}
  section:last-child {{ border-right:0; }}
  h2 {{ margin:0 0 .75rem; font-size:.8rem; text-transform:uppercase; letter-spacing:.08em; color:var(--muted); }}
  #treemap {{ display:flex; flex-wrap:wrap; gap:4px; align-content:flex-start; }}
  .cell {{ border-radius:4px; padding:6px 8px; font-size:11px; overflow:hidden; cursor:default;
           min-width:72px; min-height:48px; background:#1a2330; border:1px solid #2a3a4d; }}
  .cell.dead {{ border-color:var(--bad); }}
  .cell .p {{ display:block; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }}
  .cell .m {{ color:var(--muted); font-size:10px; }}
  #graph {{ width:100%; height:calc(100vh - 140px); background:#121820; border-radius:6px; }}
  @media (max-width:900px) {{ main {{ grid-template-columns:1fr; }} }}
</style>
</head>
<body>
<header>
  <h1>fallow-luau</h1>
  <p>Treemap by LOC (color = maintainability). Require graph on the right.</p>
</header>
<main>
  <section>
    <h2>Files</h2>
    <div id="treemap"></div>
  </section>
  <section>
    <h2>Require graph</h2>
    <svg id="graph"></svg>
  </section>
</main>
<script>
const DATA = {data_json};
const unused = new Set(DATA.unused_files || []);
const treemap = document.getElementById('treemap');
const maxLines = Math.max(1, ...DATA.nodes.map(n => n.lines||1));
function miColor(mi) {{
  if (mi == null) return '#2a3a4d';
  if (mi >= 70) return '#1e4d3a';
  if (mi >= 40) return '#5a4020';
  return '#5a2020';
}}
DATA.nodes.sort((a,b)=> (b.lines||0)-(a.lines||0)).forEach(n => {{
  const el = document.createElement('div');
  el.className = 'cell' + (unused.has(n.path) ? ' dead' : '');
  const scale = 0.55 + 0.9 * ((n.lines||1)/maxLines);
  el.style.flex = `${{scale}} 1 ${{Math.round(70*scale)}}px`;
  el.style.background = miColor(n.mi);
  el.innerHTML = `<span class="p" title="${{n.path}}">${{n.path}}</span><span class="m">${{n.lines}} LOC · MI ${{n.mi ?? '—'}} · in ${{n.fan_in}}/out ${{n.fan_out}}</span>`;
  treemap.appendChild(el);
}});

const svg = document.getElementById('graph');
const W = svg.clientWidth || 600, H = svg.clientHeight || 500;
svg.setAttribute('viewBox', `0 0 ${{W}} ${{H}}`);
const nodes = DATA.nodes.map((n,i) => {{
  const angle = (i / Math.max(1, DATA.nodes.length)) * Math.PI * 2;
  const r = Math.min(W,H)*0.36;
  return {{ ...n, x: W/2 + Math.cos(angle)*r, y: H/2 + Math.sin(angle)*r }};
}});
const byPath = Object.fromEntries(nodes.map(n => [n.path, n]));
(DATA.edges||[]).forEach(e => {{
  const a = byPath[e.from], b = byPath[e.to];
  if (!a || !b) return;
  const line = document.createElementNS('http://www.w3.org/2000/svg','line');
  line.setAttribute('x1', a.x); line.setAttribute('y1', a.y);
  line.setAttribute('x2', b.x); line.setAttribute('y2', b.y);
  line.setAttribute('stroke', '#3a4d63'); line.setAttribute('stroke-width', '1');
  svg.appendChild(line);
}});
nodes.forEach(n => {{
  const c = document.createElementNS('http://www.w3.org/2000/svg','circle');
  c.setAttribute('cx', n.x); c.setAttribute('cy', n.y);
  c.setAttribute('r', unused.has(n.path) ? 7 : 5);
  c.setAttribute('fill', unused.has(n.path) ? '#c44b4b' : '#3d9a78');
  svg.appendChild(c);
  const t = document.createElementNS('http://www.w3.org/2000/svg','text');
  t.setAttribute('x', n.x+8); t.setAttribute('y', n.y+3);
  t.setAttribute('fill', '#8b9aab'); t.setAttribute('font-size', '10');
  t.textContent = n.path.split('/').pop();
  svg.appendChild(t);
}});
</script>
</body>
</html>
"#
    ))
}

pub fn write_viz(root: &Path, out: &Path) -> Result<(), String> {
    let html = render_viz_html(root)?;
    std::fs::write(out, html).map_err(|e| format!("write {}: {e}", out.display()))?;
    Ok(())
}
