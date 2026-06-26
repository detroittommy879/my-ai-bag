use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use my_ai_bag::{
    Category, PackOptions, ScanOptions, ScanReport, builtin_tools, format_preview,
    format_scan_summary, pack_selected_tools, preview_for_selection, scan_tools,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    path::{Path, PathBuf},
};

#[derive(Debug, Parser)]
#[command(name = "aibag")]
#[command(about = "Pack AI coding tool setup folders into a local encrypted bag.")]
struct Cli {
    #[arg(long)]
    ui: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Scan(CommonArgs),
    Pack(PackArgs),
    Tools,
    Ui,
}

#[derive(Debug, Parser)]
struct CommonArgs {
    #[arg(long)]
    project_root: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = ScopeArg::Both)]
    scope: ScopeArg,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct PackArgs {
    #[arg(long)]
    project_root: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = ScopeArg::Both)]
    scope: ScopeArg,
    #[arg(long, value_delimiter = ',')]
    include: Vec<String>,
    #[arg(long)]
    all: bool,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    passphrase: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ScopeArg {
    Home,
    Project,
    Both,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.ui {
        my_ai_bag::ui::launch_ui();
        return Ok(());
    }

    match cli.command.unwrap_or(Command::Scan(CommonArgs {
        project_root: None,
        home: None,
        scope: ScopeArg::Both,
        json: false,
    })) {
        Command::Scan(args) => scan(args),
        Command::Pack(args) => pack(args),
        Command::Tools => tools(),
        Command::Ui => {
            my_ai_bag::ui::launch_ui();
            Ok(())
        }
    }
}

fn scan(args: CommonArgs) -> Result<()> {
    let report = scan_tools(&scan_options(args.home, args.project_root, args.scope)?);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", format_scan_summary(&report));
    }
    Ok(())
}

fn pack(args: PackArgs) -> Result<()> {
    let report = scan_tools(&scan_options(args.home, args.project_root, args.scope)?);
    let report = filter_report_for_includes(report, &args.include)?;
    let selected = selected_tools(&report, &args.include, args.all);
    let preview = preview_for_selection(&report, &selected);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&preview)?);
    } else {
        println!("{}", format_preview(&report, &preview));
    }

    let Some(output) = args.output else {
        eprintln!();
        eprintln!(
            "Preview only. Add --output bag.aibag and --passphrase or AIBAG_PASSPHRASE to write an encrypted bag."
        );
        return Ok(());
    };

    let passphrase = args
        .passphrase
        .or_else(|| env::var("AIBAG_PASSPHRASE").ok())
        .context("packing requires --passphrase or AIBAG_PASSPHRASE")?;

    let archive = pack_selected_tools(
        &report,
        &PackOptions {
            selected_keys: selected,
            output: output.clone(),
            passphrase,
        },
    )?;
    eprintln!(
        "Wrote encrypted bag to {} with {} file(s).",
        output.display(),
        archive.files.len()
    );
    Ok(())
}

fn tools() -> Result<()> {
    for tool in builtin_tools() {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            tool.key,
            tool.display_name,
            tool.global_skills_dir,
            tool.project_skills_dir.as_deref().unwrap_or("N/A"),
            tool.detected_if_exists
        );
    }
    Ok(())
}

fn scan_options(
    home: Option<PathBuf>,
    project_root: Option<PathBuf>,
    scope: ScopeArg,
) -> Result<ScanOptions> {
    let home_dir = home
        .or_else(dirs::home_dir)
        .context("could not determine home directory; pass --home")?;
    let project_root = project_root.unwrap_or(env::current_dir()?);
    let options = ScanOptions::new(clean_path(home_dir), clean_path(project_root));
    Ok(match scope {
        ScopeArg::Home => options.home_only(),
        ScopeArg::Project => options.project_only(),
        ScopeArg::Both => options,
    })
}

fn clean_path(path: PathBuf) -> PathBuf {
    if path == Path::new(".") {
        env::current_dir().unwrap_or(path)
    } else {
        path
    }
}

fn selected_tools(report: &my_ai_bag::ScanReport, include: &[String], all: bool) -> Vec<String> {
    if !include.is_empty() {
        return include
            .iter()
            .map(|item| item.split_once(':').map_or(item.as_str(), |(tool, _)| tool))
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }

    if all {
        return report.tools.iter().map(|tool| tool.key.clone()).collect();
    }

    report
        .tools
        .iter()
        .filter(|tool| tool.detected)
        .map(|tool| tool.key.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use my_ai_bag::{DiscoveredPath, ScanScope, ToolScan};

    #[test]
    fn include_filter_can_select_one_category_for_one_tool() {
        let report = ScanReport {
            home_dir: PathBuf::from("home"),
            project_root: PathBuf::from("project"),
            tools: vec![ToolScan {
                key: "codex".to_string(),
                display_name: "Codex".to_string(),
                detected: true,
                detected_path: PathBuf::from("home/.codex"),
                global_skills_dir: PathBuf::from("home/.codex/skills"),
                project_skills_dir: Some(PathBuf::from("project/.agents/skills")),
                found: vec![
                    discovered("home/.codex/skills", Category::Skill),
                    discovered("home/.codex/auth.json", Category::Auth),
                ],
                missing: Vec::new(),
                notes: Vec::new(),
            }],
        };

        let filtered = filter_report_for_includes(report, &["codex:skills".to_string()]).unwrap();
        let codex = filtered.tools.first().unwrap();

        assert!(codex.detected);
        assert_eq!(codex.found.len(), 1);
        assert_eq!(codex.found[0].category, Category::Skill);
        assert!(codex.missing.is_empty());
    }

    fn discovered(path: &str, category: Category) -> DiscoveredPath {
        DiscoveredPath {
            path: PathBuf::from(path),
            scope: ScanScope::Home,
            category,
            is_dir: false,
            file_count: 1,
            byte_count: 1,
        }
    }
}

fn filter_report_for_includes(mut report: ScanReport, include: &[String]) -> Result<ScanReport> {
    let filters = parse_include_filters(include)?;
    if filters.is_empty() {
        return Ok(report);
    }

    for tool in &mut report.tools {
        let Some(filter) = filters.get(tool.key.as_str()) else {
            tool.found.clear();
            tool.detected = false;
            continue;
        };

        if let IncludeFilter::Categories(categories) = filter {
            tool.found
                .retain(|found| categories.contains(&found.category));
            tool.missing.clear();
            tool.detected = !tool.found.is_empty();
        }
    }

    Ok(report)
}

#[derive(Debug)]
enum IncludeFilter {
    All,
    Categories(BTreeSet<Category>),
}

fn parse_include_filters(include: &[String]) -> Result<BTreeMap<&str, IncludeFilter>> {
    let mut filters = BTreeMap::new();

    for item in include {
        let Some((tool, category)) = item.split_once(':') else {
            filters.insert(item.as_str(), IncludeFilter::All);
            continue;
        };

        let category = parse_category(category)?;
        match filters.entry(tool) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(IncludeFilter::Categories(BTreeSet::from([category])));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => match entry.get_mut() {
                IncludeFilter::All => {}
                IncludeFilter::Categories(categories) => {
                    categories.insert(category);
                }
            },
        }
    }

    Ok(filters)
}

fn parse_category(value: &str) -> Result<Category> {
    match value.to_ascii_lowercase().as_str() {
        "skill" | "skills" => Ok(Category::Skill),
        "setting" | "settings" | "config" | "configs" => Ok(Category::Setting),
        "auth" | "secret" | "secrets" => Ok(Category::Auth),
        "mcp" => Ok(Category::Mcp),
        _ => anyhow::bail!(
            "unknown include category '{value}', expected skills, settings, auth, or mcp"
        ),
    }
}
