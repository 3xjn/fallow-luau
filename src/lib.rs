//! fallow-luau: Fallow-shaped codebase intelligence for Luau.
//!
//! Algorithms follow the published Fallow health formulas:
//! <https://docs.fallow.tools/explanations/health>
//! Parity map: `docs/parity.md`.

mod audit;
mod complexity;
mod dead_code;
mod discover;
mod dupes;
mod explain;
mod graph;
mod health;
mod mcp;
mod meta;
mod project;
mod report;
mod resolve;

pub use audit::{analyze_audit, AuditOptions, AuditReport, AuditVerdict};
pub use complexity::{analyze_functions, FunctionMetrics, RiskBin, RiskProfile};
pub use dead_code::{analyze_dead_code, DeadCodeOptions, DeadCodeReport, DeadKind};
pub use discover::{discover_files, is_luau_source, is_test_path};
pub use dupes::{analyze_dupes, CloneGroup, DupesOptions, DupesReport};
pub use explain::{explain_rule, known_rules};
pub use graph::{build_require_graph, RequireEdge, RequireGraph, RequireKind};
pub use health::{
    analyze_health, ChurnTrend, Effort, FileScore, HealthOptions, HealthReport, Hotspot,
    RefactorTarget, TargetCategory, UnitProfiles,
};
pub use mcp::run_mcp_stdio;
pub use meta::{dead_code_meta, dupes_meta, health_meta};
pub use project::{analyze_project, Project, ProjectOptions};
pub use report::{list_report, schema_manifest};
pub use resolve::{resolve_require, ResolvedRequire};
