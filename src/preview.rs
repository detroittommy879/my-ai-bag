use crate::scan::{Category, ScanReport};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackPreview {
    pub selected_tools: Vec<String>,
    pub detected_tools: Vec<String>,
    pub unique_folders_to_include: Vec<PathBuf>,
    pub missing_folders: Vec<PathBuf>,
    pub duplicate_shared_folders: Vec<DuplicateFolder>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateFolder {
    pub path: PathBuf,
    pub tools: Vec<String>,
}

pub fn preview_for_selection(report: &ScanReport, selected_keys: &[String]) -> PackPreview {
    let selected: BTreeSet<&str> = selected_keys.iter().map(String::as_str).collect();
    let mut detected_tools = Vec::new();
    let mut unique_folders_to_include = BTreeSet::new();
    let mut missing_folders = BTreeSet::new();
    let mut folder_owners: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    let mut notes = vec![
        "No upload happens. Preview and export are local only.".to_string(),
        "Secret-looking files are listed by path only. File contents are never printed."
            .to_string(),
    ];

    for tool in &report.tools {
        if tool.detected {
            detected_tools.push(format!("{} ({})", tool.display_name, tool.key));
        }

        if !selected.contains(tool.key.as_str()) {
            continue;
        }

        for found in &tool.found {
            if found.is_dir {
                unique_folders_to_include.insert(found.path.clone());
                folder_owners
                    .entry(found.path.clone())
                    .or_default()
                    .insert(tool.display_name.clone());
            } else if let Some(parent) = found.path.parent() {
                unique_folders_to_include.insert(parent.to_path_buf());
                folder_owners
                    .entry(parent.to_path_buf())
                    .or_default()
                    .insert(tool.display_name.clone());
            }
        }

        for missing in &tool.missing {
            missing_folders.insert(missing.clone());
        }

        for note in &tool.notes {
            notes.push(format!("{}: {note}", tool.display_name));
        }
    }

    notes.push(
        "CLI encrypted export is implemented with Argon2id and XChaCha20-Poly1305.".to_string(),
    );
    notes.push("UI export is intentionally preview-only in this prototype; use aibag pack --output for encrypted export.".to_string());

    let duplicate_shared_folders = folder_owners
        .into_iter()
        .filter_map(|(path, tools)| {
            if tools.len() > 1 {
                Some(DuplicateFolder {
                    path,
                    tools: tools.into_iter().collect(),
                })
            } else {
                None
            }
        })
        .collect();

    PackPreview {
        selected_tools: selected_keys.to_vec(),
        detected_tools,
        unique_folders_to_include: unique_folders_to_include.into_iter().collect(),
        missing_folders: missing_folders.into_iter().collect(),
        duplicate_shared_folders,
        notes,
    }
}

pub fn format_preview(report: &ScanReport, preview: &PackPreview) -> String {
    let mut lines = vec![
        "My AI Bag pack preview".to_string(),
        format!("Home: {}", report.home_dir.display()),
        format!("Project root: {}", report.project_root.display()),
        String::new(),
        "Detected tools:".to_string(),
    ];

    if preview.detected_tools.is_empty() {
        lines.push("- none".to_string());
    } else {
        lines.extend(
            preview
                .detected_tools
                .iter()
                .map(|tool| format!("- {tool}")),
        );
    }

    lines.push(String::new());
    lines.push("Selected tools:".to_string());
    if preview.selected_tools.is_empty() {
        lines.push("- none selected".to_string());
    } else {
        for key in &preview.selected_tools {
            if let Some(tool) = report.tools.iter().find(|tool| &tool.key == key) {
                lines.push(format!("- {} ({})", tool.display_name, tool.key));
                for found in &tool.found {
                    lines.push(format!(
                        "  - {:?}: {} ({} file(s), {} byte(s))",
                        found.category,
                        found.path.display(),
                        found.file_count,
                        found.byte_count
                    ));
                }
            }
        }
    }

    lines.push(String::new());
    lines.push("Unique folders to include:".to_string());
    push_path_list(&mut lines, &preview.unique_folders_to_include);

    lines.push(String::new());
    lines.push("Missing folders:".to_string());
    push_path_list(&mut lines, &preview.missing_folders);

    lines.push(String::new());
    lines.push("Duplicate/shared folders:".to_string());
    if preview.duplicate_shared_folders.is_empty() {
        lines.push("- none".to_string());
    } else {
        for duplicate in &preview.duplicate_shared_folders {
            lines.push(format!(
                "- {} used by {}",
                duplicate.path.display(),
                duplicate.tools.join(", ")
            ));
        }
    }

    lines.push(String::new());
    lines.push("Notes:".to_string());
    lines.extend(preview.notes.iter().map(|note| format!("- {note}")));

    lines.join("\n")
}

fn push_path_list(lines: &mut Vec<String>, paths: &[PathBuf]) {
    if paths.is_empty() {
        lines.push("- none".to_string());
    } else {
        lines.extend(paths.iter().map(|path| format!("- {}", path.display())));
    }
}

pub fn category_label(category: Category) -> &'static str {
    match category {
        Category::Skill => "skills",
        Category::Setting => "settings",
        Category::Auth => "auth",
        Category::Mcp => "mcp",
    }
}

#[cfg(test)]
mod tests {
    use crate::{ScanOptions, scan_tools};
    use std::fs;

    use super::*;

    #[test]
    fn preview_reports_shared_project_skill_folder() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(project.join(".agents/skills/team")).unwrap();
        fs::write(project.join(".agents/skills/team/SKILL.md"), "team skill").unwrap();

        let report = scan_tools(&ScanOptions::new(home, project));
        let selected = vec!["cursor".to_string(), "codex".to_string()];
        let preview = preview_for_selection(&report, &selected);

        assert!(
            preview
                .duplicate_shared_folders
                .iter()
                .any(|folder| folder.path.ends_with(".agents/skills"))
        );
    }
}
