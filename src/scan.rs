use crate::catalog::{ToolCatalogEntry, builtin_tools};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Skill,
    Setting,
    Auth,
    Mcp,
    AgentFolder,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScanScope {
    Home,
    Project,
}

impl ScanScope {
    pub fn label(self) -> &'static str {
        match self {
            ScanScope::Home => "home",
            ScanScope::Project => "project",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredPath {
    pub path: PathBuf,
    pub scope: ScanScope,
    pub category: Category,
    pub is_dir: bool,
    pub file_count: usize,
    pub byte_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRoot {
    pub path: PathBuf,
    pub scope: ScanScope,
    pub file_count: usize,
    pub byte_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolScan {
    pub key: String,
    pub display_name: String,
    pub detected: bool,
    pub detected_path: PathBuf,
    pub global_skills_dir: PathBuf,
    pub project_skills_dir: Option<PathBuf>,
    pub roots: Vec<AgentRoot>,
    pub found: Vec<DiscoveredPath>,
    pub missing: Vec<PathBuf>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub home_dir: PathBuf,
    pub project_root: PathBuf,
    pub tools: Vec<ToolScan>,
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub home_dir: PathBuf,
    pub project_root: PathBuf,
    pub include_home: bool,
    pub include_project: bool,
    pub catalog: Vec<ToolCatalogEntry>,
}

impl ScanOptions {
    pub fn new(home_dir: PathBuf, project_root: PathBuf) -> Self {
        Self {
            home_dir,
            project_root,
            include_home: true,
            include_project: true,
            catalog: builtin_tools(),
        }
    }

    pub fn home_only(mut self) -> Self {
        self.include_home = true;
        self.include_project = false;
        self
    }

    pub fn project_only(mut self) -> Self {
        self.include_home = false;
        self.include_project = true;
        self
    }
}

pub fn scan_tools(options: &ScanOptions) -> ScanReport {
    let tools = options
        .catalog
        .iter()
        .map(|entry| {
            scan_tool(
                entry,
                &options.home_dir,
                &options.project_root,
                options.include_home,
                options.include_project,
            )
        })
        .collect();

    ScanReport {
        home_dir: options.home_dir.clone(),
        project_root: options.project_root.clone(),
        tools,
    }
}

pub fn format_scan_summary(report: &ScanReport) -> String {
    let detected = report.tools.iter().filter(|tool| tool.detected).count();
    let mut lines = vec![
        "My AI Bag scan".to_string(),
        format!("Home: {}", report.home_dir.display()),
        format!("Project root: {}", report.project_root.display()),
        format!("Detected tools: {detected}/{}", report.tools.len()),
        String::new(),
    ];

    for tool in &report.tools {
        let status = if tool.detected { "detected" } else { "missing" };
        let found_count = tool.found.len();
        let scopes = tool
            .found
            .iter()
            .map(|found| found.scope.label())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        let scopes = if scopes.is_empty() {
            "none found".to_string()
        } else {
            scopes
        };
        lines.push(format!(
            "- {} ({}) - {} item(s), {}",
            tool.display_name, status, found_count, scopes
        ));
    }

    lines.join("\n")
}

fn scan_tool(
    entry: &ToolCatalogEntry,
    home_dir: &Path,
    project_root: &Path,
    include_home: bool,
    include_project: bool,
) -> ToolScan {
    let default_detected_path = home_dir.join(&entry.detected_if_exists);
    let global_skills_dir = home_dir.join(&entry.global_skills_dir);
    let project_skills_dir = entry
        .project_skills_dir
        .as_deref()
        .map(|dir| project_root.join(dir));

    let mut found = Vec::new();
    let mut roots = Vec::new();
    let mut missing = Vec::new();
    let mut notes = Vec::new();

    if include_home {
        if is_home_agents_path(&entry.detected_if_exists)
            || is_home_agents_path(&entry.global_skills_dir)
        {
            notes.push("Skipped ~/.agents in home scope; .agents is treated as a project folder by default.".to_string());
        } else {
            push_skill_dir(
                &mut found,
                &mut missing,
                &global_skills_dir,
                ScanScope::Home,
            );
            for relative in entry.effective_home_roots() {
                let home_root = home_dir.join(relative);
                if home_root.exists() {
                    push_agent_root(&mut roots, &home_root, ScanScope::Home);
                    collect_config_candidates(&home_root, ScanScope::Home, &mut found);
                } else {
                    missing.push(home_root);
                }
            }
        }
    }

    if include_project {
        if let Some(project_skills_dir) = &project_skills_dir {
            push_skill_dir(
                &mut found,
                &mut missing,
                project_skills_dir,
                ScanScope::Project,
            );
        } else {
            notes.push("No project skills directory is known for this tool.".to_string());
        }

        for relative in entry.effective_project_roots() {
            let project_agent_root = project_root.join(relative);
            if project_agent_root.exists() {
                push_agent_root(&mut roots, &project_agent_root, ScanScope::Project);
                collect_config_candidates(&project_agent_root, ScanScope::Project, &mut found);
            } else {
                missing.push(project_agent_root);
            }
        }
    }

    found.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.category.cmp(&right.category))
    });
    found.dedup_by(|left, right| left.path == right.path && left.category == right.category);
    missing.sort();
    missing.dedup();
    roots.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.scope.cmp(&right.scope))
    });
    roots.dedup_by(|left, right| left.path == right.path && left.scope == right.scope);

    ToolScan {
        key: entry.key.clone(),
        display_name: entry.display_name.clone(),
        detected: !found.is_empty() || !roots.is_empty(),
        detected_path: default_detected_path,
        global_skills_dir,
        project_skills_dir,
        roots,
        found,
        missing,
        notes,
    }
}

fn push_agent_root(roots: &mut Vec<AgentRoot>, path: &Path, scope: ScanScope) {
    let (file_count, byte_count) = summarize_dir(path, 12);
    roots.push(AgentRoot {
        path: path.to_path_buf(),
        scope,
        file_count,
        byte_count,
    });
}

fn is_home_agents_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized == ".agents" || normalized.starts_with(".agents/")
}

fn push_skill_dir(
    found: &mut Vec<DiscoveredPath>,
    missing: &mut Vec<PathBuf>,
    path: &Path,
    scope: ScanScope,
) {
    if path.exists() {
        let (file_count, byte_count) = summarize_dir(path, 8);
        found.push(DiscoveredPath {
            path: path.to_path_buf(),
            scope,
            category: Category::Skill,
            is_dir: true,
            file_count,
            byte_count,
        });
    } else {
        missing.push(path.to_path_buf());
    }
}

fn collect_config_candidates(root: &Path, scope: ScanScope, found: &mut Vec<DiscoveredPath>) {
    let mut seen = BTreeSet::new();
    visit_limited(root, 0, 3, &mut |path, is_dir| {
        if path == root {
            return;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            return;
        };
        let lowered = name.to_ascii_lowercase();

        let category = if looks_like_auth(&lowered, is_dir) {
            Some(Category::Auth)
        } else if looks_like_mcp(&lowered) {
            Some(Category::Mcp)
        } else if looks_like_setting(&lowered) {
            Some(Category::Setting)
        } else {
            None
        };

        if let Some(category) = category {
            let key = (path.to_path_buf(), category);
            if seen.insert(key) {
                let (file_count, byte_count) = if is_dir {
                    summarize_dir(path, 6)
                } else {
                    (
                        1,
                        fs::metadata(path)
                            .map(|metadata| metadata.len())
                            .unwrap_or(0),
                    )
                };
                found.push(DiscoveredPath {
                    path: path.to_path_buf(),
                    scope,
                    category,
                    is_dir,
                    file_count,
                    byte_count,
                });
            }
        }
    });
}

fn looks_like_auth(name: &str, is_dir: bool) -> bool {
    if is_dir {
        return matches!(
            name,
            "auth" | "credentials" | "credential" | "secrets" | "secret" | "tokens" | "token"
        );
    }

    if name == ".env" || name.starts_with(".env.") {
        return true;
    }

    let stem = name
        .split_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(name)
        .trim_matches(|ch: char| ch == '_' || ch == '-');

    matches!(
        stem,
        "auth"
            | "credential"
            | "credentials"
            | "secret"
            | "secrets"
            | "session"
            | "sessions"
            | "token"
            | "tokens"
            | "oauth"
            | "apikey"
            | "api_key"
    )
}

fn looks_like_mcp(name: &str) -> bool {
    name.contains("mcp")
}

fn looks_like_setting(name: &str) -> bool {
    name == "settings.json"
        || name == "config.json"
        || name == "config.toml"
        || name == "config.yaml"
        || name == "config.yml"
        || name == "settings.toml"
        || name == "settings.yaml"
        || name == "settings.yml"
}

fn summarize_dir(path: &Path, max_depth: usize) -> (usize, u64) {
    let mut file_count = 0;
    let mut byte_count = 0;
    visit_limited(path, 0, max_depth, &mut |candidate, is_dir| {
        if !is_dir {
            file_count += 1;
            byte_count += fs::metadata(candidate)
                .map(|metadata| metadata.len())
                .unwrap_or(0);
        }
    });
    (file_count, byte_count)
}

pub fn visit_limited(
    path: &Path,
    depth: usize,
    max_depth: usize,
    visitor: &mut impl FnMut(&Path, bool),
) {
    if fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return;
    }
    let is_dir = path.is_dir();
    visitor(path, is_dir);
    if !is_dir || depth >= max_depth {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        visit_limited(&entry.path(), depth + 1, max_depth, visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_skills_settings_auth_and_mcp_without_reading_contents() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        fs::create_dir_all(home.join(".codex/skills/rust-helper")).unwrap();
        fs::create_dir_all(project.join(".agents/skills/shared")).unwrap();
        fs::write(
            home.join(".codex/skills/rust-helper/SKILL.md"),
            "skill docs",
        )
        .unwrap();
        fs::write(
            project.join(".agents/skills/shared/SKILL.md"),
            "shared skill",
        )
        .unwrap();
        fs::write(home.join(".codex/settings.json"), "{\"theme\":\"dark\"}").unwrap();
        fs::write(
            home.join(".codex/auth.json"),
            "{\"token\":\"sk-test-secret\"}",
        )
        .unwrap();
        fs::write(home.join(".codex/mcp.json"), "{\"servers\":{}}").unwrap();

        let report = scan_tools(&ScanOptions::new(home.clone(), project));
        let codex = report
            .tools
            .iter()
            .find(|tool| tool.key == "codex")
            .unwrap();

        assert!(codex.detected);
        assert!(
            codex
                .found
                .iter()
                .any(|item| item.category == Category::Skill)
        );
        assert!(
            codex
                .found
                .iter()
                .any(|item| item.category == Category::Setting)
        );
        assert!(
            codex
                .found
                .iter()
                .any(|item| item.category == Category::Auth)
        );
        assert!(
            codex
                .found
                .iter()
                .any(|item| item.category == Category::Mcp)
        );

        let summary = format_scan_summary(&report);
        assert!(!summary.contains("sk-test-secret"));
    }

    #[test]
    fn default_scan_ignores_home_agents_but_includes_project_agents() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        fs::create_dir_all(home.join(".agents/skills/accidental")).unwrap();
        fs::write(
            home.join(".agents/skills/accidental/SKILL.md"),
            "do not include home agents by default",
        )
        .unwrap();
        fs::create_dir_all(project.join(".agents/skills/team")).unwrap();
        fs::write(project.join(".agents/skills/team/SKILL.md"), "team skill").unwrap();

        let report = scan_tools(&ScanOptions::new(home, project));
        let cline = report
            .tools
            .iter()
            .find(|tool| tool.key == "cline")
            .unwrap();

        assert!(cline.detected);
        assert!(
            cline
                .found
                .iter()
                .any(|item| item.scope == ScanScope::Project
                    && item.path.ends_with(".agents/skills"))
        );
        assert!(
            !cline
                .found
                .iter()
                .any(|item| item.scope == ScanScope::Home && item.path.ends_with(".agents/skills"))
        );
        assert!(
            cline
                .notes
                .iter()
                .any(|note| note.contains("Skipped ~/.agents"))
        );
    }

    #[test]
    fn project_only_scan_finds_agent_specific_project_root() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        fs::create_dir_all(project.join(".codex")).unwrap();
        fs::write(project.join(".codex/settings.json"), "{\"project\":true}").unwrap();

        let report = scan_tools(&ScanOptions::new(home, project).project_only());
        let codex = report
            .tools
            .iter()
            .find(|tool| tool.key == "codex")
            .unwrap();

        assert!(codex.detected);
        assert!(
            codex
                .found
                .iter()
                .any(|item| item.scope == ScanScope::Project
                    && item.category == Category::Setting
                    && item.path.ends_with(".codex/settings.json"))
        );
    }
}
