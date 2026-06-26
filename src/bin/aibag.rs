use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use my_ai_bag::{
    PackOptions, ScanOptions, builtin_tools, format_preview, format_scan_summary,
    pack_selected_tools, preview_for_selection, scan_tools,
};
use std::{
    env,
    path::{Path, PathBuf},
};

#[derive(Debug, Parser)]
#[command(name = "aibag")]
#[command(about = "Pack AI coding tool setup folders into a local encrypted bag.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Scan(CommonArgs),
    Pack(PackArgs),
    Tools,
}

#[derive(Debug, Parser)]
struct CommonArgs {
    #[arg(long)]
    project_root: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Parser)]
struct PackArgs {
    #[arg(long)]
    project_root: Option<PathBuf>,
    #[arg(long)]
    home: Option<PathBuf>,
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Scan(CommonArgs {
        project_root: None,
        home: None,
        json: false,
    })) {
        Command::Scan(args) => scan(args),
        Command::Pack(args) => pack(args),
        Command::Tools => tools(),
    }
}

fn scan(args: CommonArgs) -> Result<()> {
    let report = scan_tools(&scan_options(args.home, args.project_root)?);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("{}", format_scan_summary(&report));
    }
    Ok(())
}

fn pack(args: PackArgs) -> Result<()> {
    let report = scan_tools(&scan_options(args.home, args.project_root)?);
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

fn scan_options(home: Option<PathBuf>, project_root: Option<PathBuf>) -> Result<ScanOptions> {
    let home_dir = home
        .or_else(dirs::home_dir)
        .context("could not determine home directory; pass --home")?;
    let project_root = project_root.unwrap_or(env::current_dir()?);
    Ok(ScanOptions::new(
        clean_path(home_dir),
        clean_path(project_root),
    ))
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
        return include.to_vec();
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
