use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCatalogEntry {
    pub key: String,
    pub display_name: String,
    pub global_skills_dir: String,
    pub project_skills_dir: Option<String>,
    pub detected_if_exists: String,
}

pub fn builtin_tools() -> Vec<ToolCatalogEntry> {
    parse_catalog(include_str!("../agents.toml"))
        .expect("bundled agents.toml should always parse")
        .agents
}

#[derive(Debug, Clone, Deserialize)]
struct ToolCatalog {
    agents: Vec<ToolCatalogEntry>,
}

fn parse_catalog(contents: &str) -> Result<ToolCatalog, toml::de::Error> {
    toml::from_str(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_editable_toml_and_has_stable_keys() {
        let tools = builtin_tools();

        assert!(tools.iter().any(|tool| tool.key == "codex"));
        assert!(tools.iter().any(|tool| tool.key == "claude_code"));
        assert!(tools.iter().all(|tool| !tool.display_name.is_empty()));
    }
}
