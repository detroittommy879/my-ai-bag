use crate::scan::{Category, ScanReport, ScanScope, visit_limited};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLibrary {
    pub version: u32,
    pub packages: Vec<SkillPackage>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPackage {
    pub id: String,
    pub name: String,
    pub library_dir: String,
    pub file_count: usize,
    pub byte_count: u64,
    pub tool_keys: Vec<String>,
    pub sources: Vec<SkillSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SkillSource {
    pub path: PathBuf,
    pub scope: ScanScope,
}

pub fn discover_skills(report: &ScanReport, selected_keys: &[String]) -> Result<SkillLibrary> {
    let selected = selected_keys
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut source_owners: BTreeMap<(PathBuf, ScanScope), BTreeSet<String>> = BTreeMap::new();

    for tool in report
        .tools
        .iter()
        .filter(|tool| selected.contains(tool.key.as_str()))
    {
        for found in &tool.found {
            if found.category != Category::Skill || !found.is_dir {
                continue;
            }
            for package_dir in find_skill_packages(&found.path) {
                source_owners
                    .entry((package_dir, found.scope))
                    .or_default()
                    .insert(tool.key.clone());
            }
        }
    }

    let mut by_hash: BTreeMap<String, SkillPackage> = BTreeMap::new();
    for ((source_path, scope), tool_keys) in source_owners {
        let (id, file_count, byte_count) = hash_directory(&source_path)?;
        let name = source_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("skill")
            .to_string();
        let library_dir = format!("{}--{}", slug(&name), &id[..12]);
        let package = by_hash.entry(id.clone()).or_insert_with(|| SkillPackage {
            id,
            name,
            library_dir,
            file_count,
            byte_count,
            tool_keys: Vec::new(),
            sources: Vec::new(),
        });
        package.tool_keys.extend(tool_keys);
        package.sources.push(SkillSource {
            path: source_path,
            scope,
        });
        package.tool_keys.sort();
        package.tool_keys.dedup();
        package.sources.sort();
        package.sources.dedup();
    }

    Ok(SkillLibrary {
        version: 1,
        packages: by_hash.into_values().collect(),
        notes: vec![
            "Skills are deduplicated by SHA-256 content hash.".to_string(),
            "Central collection copies complete skill folders, including scripts.".to_string(),
            "Review skills before running them; collected scripts are not sandboxed.".to_string(),
            "Enabling, disabling, and installing skills into agents is not implemented yet."
                .to_string(),
        ],
    })
}

pub fn write_skill_library(library: &SkillLibrary, store: &Path) -> Result<()> {
    fs::create_dir_all(store)
        .with_context(|| format!("failed to create skill store {}", store.display()))?;

    for package in &library.packages {
        let Some(source) = package.sources.first() else {
            continue;
        };
        copy_directory(&source.path, &store.join(&package.library_dir))?;
    }

    let index = serde_json::to_vec_pretty(library)?;
    fs::write(store.join("index.json"), index)
        .with_context(|| format!("failed to write skill index in {}", store.display()))
}

pub fn format_skill_library(library: &SkillLibrary) -> String {
    let mut lines = vec![
        "My AI Bag central skills preview".to_string(),
        format!("Unique skill packages: {}", library.packages.len()),
        String::new(),
    ];

    for package in &library.packages {
        lines.push(format!(
            "- {} | {} | {} file(s), {} byte(s) | tools: {}",
            package.name,
            &package.id[..12],
            package.file_count,
            package.byte_count,
            package.tool_keys.join(", ")
        ));
        for source in &package.sources {
            lines.push(format!(
                "  - {}: {}",
                source.scope.label(),
                source.path.display()
            ));
        }
    }
    if library.packages.is_empty() {
        lines.push("- none".to_string());
    }

    lines.push(String::new());
    lines.push("Notes:".to_string());
    lines.extend(library.notes.iter().map(|note| format!("- {note}")));
    lines.join("\n")
}

fn find_skill_packages(root: &Path) -> Vec<PathBuf> {
    let mut packages = BTreeSet::new();
    visit_limited(root, 0, 8, &mut |candidate, is_dir| {
        if is_dir && contains_skill_markdown(candidate) {
            packages.insert(candidate.to_path_buf());
        }
    });
    packages.into_iter().collect()
}

fn contains_skill_markdown(path: &Path) -> bool {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
        })
}

fn regular_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    visit_limited(root, 0, 12, &mut |path, is_dir| {
        if !is_dir {
            files.push(path.to_path_buf());
        }
    });
    files.sort();
    files
}

fn hash_directory(root: &Path) -> Result<(String, usize, u64)> {
    let files = regular_files(root);
    if files.is_empty() {
        return Err(anyhow!("skill folder is empty: {}", root.display()));
    }

    let mut hasher = Sha256::new();
    let mut byte_count = 0;
    for path in &files {
        let relative = path.strip_prefix(root).unwrap_or(path);
        let normalized = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read skill file {}", path.display()))?;
        hasher.update(normalized.as_bytes());
        hasher.update([0]);
        hasher.update(&bytes);
        hasher.update([0]);
        byte_count += bytes.len() as u64;
    }

    Ok((format!("{:x}", hasher.finalize()), files.len(), byte_count))
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    for file in regular_files(source) {
        let relative = file.strip_prefix(source).unwrap_or(&file);
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&file, &target).with_context(|| {
            format!(
                "failed to copy skill file {} to {}",
                file.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}

fn slug(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('-') {
            output.push('-');
        }
    }
    let output = output.trim_matches('-');
    if output.is_empty() {
        "skill".to_string()
    } else {
        output.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScanOptions, scan_tools};

    #[test]
    fn deduplicates_identical_skills_and_writes_central_index() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        for root in [
            home.join(".codex/skills/demo"),
            home.join(".claude/skills/demo"),
        ] {
            fs::create_dir_all(root.join("scripts")).unwrap();
            fs::write(root.join("SKILL.md"), "# Demo").unwrap();
            fs::write(root.join("scripts/run.ps1"), "Write-Output demo").unwrap();
        }
        fs::create_dir_all(&project).unwrap();

        let report = scan_tools(&ScanOptions::new(home, project).home_only());
        let library =
            discover_skills(&report, &["codex".to_string(), "claude_code".to_string()]).unwrap();
        assert_eq!(library.packages.len(), 1);
        assert_eq!(library.packages[0].sources.len(), 2);

        let store = temp.path().join("central-skills");
        write_skill_library(&library, &store).unwrap();
        assert!(store.join("index.json").is_file());
        assert!(
            store
                .join(&library.packages[0].library_dir)
                .join("SKILL.md")
                .is_file()
        );
    }
}
