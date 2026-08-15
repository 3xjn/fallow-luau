//! Project config: `.fallow-luau.json` or `fallow-luau.toml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub entry: Vec<String>,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub health: HealthConfig,
    #[serde(default)]
    pub duplicates: DupesConfig,
    #[serde(default)]
    pub boundaries: Vec<BoundaryZone>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    #[serde(default = "default_max_cyclomatic")]
    pub max_cyclomatic: u32,
    #[serde(default = "default_max_cognitive")]
    pub max_cognitive: u32,
    #[serde(default = "default_max_unit_size")]
    pub max_unit_size: usize,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            max_cyclomatic: default_max_cyclomatic(),
            max_cognitive: default_max_cognitive(),
            max_unit_size: default_max_unit_size(),
        }
    }
}

fn default_max_cyclomatic() -> u32 {
    20
}
fn default_max_cognitive() -> u32 {
    15
}
fn default_max_unit_size() -> usize {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DupesConfig {
    #[serde(default = "default_min_tokens")]
    pub min_tokens: usize,
    #[serde(default = "default_min_lines")]
    pub min_lines: usize,
}

impl Default for DupesConfig {
    fn default() -> Self {
        Self {
            min_tokens: default_min_tokens(),
            min_lines: default_min_lines(),
        }
    }
}

fn default_min_tokens() -> usize {
    30
}
fn default_min_lines() -> usize {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryZone {
    pub name: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub allow_imports_from: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedConfig {
    pub path: Option<String>,
    pub config: Config,
}

pub fn config_candidates(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join(".fallow-luau.json"),
        root.join("fallow-luau.toml"),
        root.join(".fallow-luau.toml"),
    ]
}

pub fn load_config(root: &Path) -> Result<ResolvedConfig, String> {
    for path in config_candidates(root) {
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        let config = if path.extension().and_then(|e| e.to_str()) == Some("json") {
            serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?
        } else {
            toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?
        };
        return Ok(ResolvedConfig {
            path: Some(path.display().to_string()),
            config,
        });
    }
    Ok(ResolvedConfig {
        path: None,
        config: Config::default(),
    })
}

pub fn init_config(root: &Path, format: InitFormat) -> Result<PathBuf, String> {
    let (path, body) = match format {
        InitFormat::Json => {
            let path = root.join(".fallow-luau.json");
            let body = serde_json::to_string_pretty(&Config::default())
                .map_err(|e| e.to_string())?
                + "\n";
            (path, body)
        }
        InitFormat::Toml => {
            let path = root.join("fallow-luau.toml");
            let body = toml::to_string_pretty(&Config::default()).map_err(|e| e.to_string())?;
            (path, body)
        }
    };
    if path.exists() {
        return Err(format!("already exists: {}", path.display()));
    }
    std::fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

#[derive(Debug, Clone, Copy)]
pub enum InitFormat {
    Json,
    Toml,
}
