use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCatalogEntry {
    pub key: String,
    pub display_name: String,
    pub global_skills_dir: String,
    pub project_skills_dir: Option<String>,
    pub detected_if_exists: String,
    #[serde(default)]
    pub home_roots: Vec<String>,
    #[serde(default)]
    pub project_roots: Vec<String>,
}

impl ToolCatalogEntry {
    pub fn effective_home_roots(&self) -> Vec<String> {
        if self.home_roots.is_empty() {
            vec![self.detected_if_exists.clone()]
        } else {
            self.home_roots.clone()
        }
    }

    pub fn effective_project_roots(&self) -> Vec<String> {
        if !self.project_roots.is_empty() {
            return self.project_roots.clone();
        }

        let mut roots = vec![self.detected_if_exists.clone()];
        if let Some(skills) = &self.project_skills_dir {
            let normalized = skills.replace('\\', "/");
            if let Some(root) = normalized.split('/').next()
                && !root.is_empty()
            {
                roots.push(root.to_string());
            }
        }
        roots.sort();
        roots.dedup();
        roots
    }
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
