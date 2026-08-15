//! fallow-luau: Fallow-shaped codebase intelligence for Luau.
//!
//! Algorithms follow the published Fallow health formulas:
//! <https://docs.fallow.tools/explanations/health>

mod complexity;
mod discover;
mod graph;
mod health;
mod meta;
mod project;
mod report;
mod resolve;

pub use complexity::{analyze_functions, FunctionMetrics, RiskBin, RiskProfile};
pub use discover::{discover_files, is_luau_source, is_test_path};
pub use graph::{build_require_graph, RequireEdge, RequireGraph, RequireKind};
pub use health::{
    analyze_health, ChurnTrend, Effort, FileScore, HealthOptions, HealthReport, Hotspot,
    RefactorTarget, TargetCategory, UnitProfiles,
};
pub use meta::health_meta;
pub use project::{analyze_project, Project, ProjectOptions};
pub use report::{list_report, schema_manifest};
pub use resolve::{resolve_require, ResolvedRequire};
