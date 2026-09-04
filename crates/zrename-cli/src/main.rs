use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use zrename_core::execute::{execute, ConflictPolicy, ExecuteOptions};
use zrename_core::journal::{self, Journal, UndoOptions};
use zrename_core::metadata::LazyMetadata;
use zrename_core::plan::{build_plan, PlanOptions};
use zrename_core::presets::{self, Preset};
use zrename_core::scan::scan;
use zrename_core::{export, RuleSpec};

#[derive(Parser)]
#[command(
    name = "zrename",
    version,
    about = "Bulk rename with a rule pipeline, a live plan and a real undo"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run(RunArgs),

    Presets,

    InitPresets,

    History {
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },

    Undo {
        id: Option<String>,

        #[arg(long)]
        force: bool,

        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Parser)]
struct RunArgs {
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    #[arg(long)]
    preset: String,

    #[arg(long)]
    dry_run: bool,

    #[arg(long)]
    apply: bool,

    #[arg(long)]
    recursive: bool,

    #[arg(long)]
    max_depth: Option<usize>,

    #[arg(long, value_delimiter = ',')]
    ext: Vec<String>,

    #[arg(long, value_enum, default_value = "stop")]
    on_conflict: Conflict,

    #[arg(long)]
    paranoid: bool,

    #[arg(long)]
    csv: Option<PathBuf>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Conflict {
    Stop,
    Skip,
    Suffix,
    Overwrite,
}

impl From<Conflict> for ConflictPolicy {
    fn from(c: Conflict) -> Self {
        match c {
            Conflict::Stop => ConflictPolicy::Stop,
            Conflict::Skip => ConflictPolicy::Skip,
            Conflict::Suffix => ConflictPolicy::Suffix,
            Conflict::Overwrite => ConflictPolicy::Overwrite,
        }
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Run(args) => run(args),
        Command::Presets => list_presets(),
        Command::InitPresets => init_presets(),
        Command::History { limit } => history(limit),
        Command::Undo { id, force, dry_run } => undo(id, force, dry_run),
    }
}

fn run(args: RunArgs) -> Result<()> {
    let preset = load_preset(&args.preset)?;
    let rules: Vec<RuleSpec> = preset.rules.clone();

    let mut scan_opts = preset.scan.clone().unwrap_or_default();
    if args.recursive {
        scan_opts.recursive = true;
    }
    if let Some(d) = args.max_depth {
        scan_opts.recursive = true;
        scan_opts.max_depth = Some(d);
    }
    if !args.ext.is_empty() {
        scan_opts.extensions = args.ext.clone();
    }

    let entries = scan(&args.paths, &scan_opts).context("scanning")?;
    if entries.is_empty() {
        println!("Nothing matched.");
        return Ok(());
    }

    let root = args.paths.first().cloned().unwrap_or_default();
    let conflict = preset
        .conflict
        .filter(|_| matches!(args.on_conflict, Conflict::Stop))
        .unwrap_or(args.on_conflict.into());
    let opts = PlanOptions {
        conflict,
        ..PlanOptions::for_path(&root)
    };

    let meta = LazyMetadata::new();
    let plan = build_plan(&entries, &rules, &meta, &opts).context("planning")?;

    if let Some(path) = &args.csv {
        std::fs::write(path, export::to_csv(&plan)?)
            .with_context(|| format!("writing {}", path.display()))?;
        println!("Wrote {}", path.display());
    } else {
        print_plan(&plan);
    }
    println!("\n{}", export::summary_line(&plan));

    if !args.apply || args.dry_run {
        println!("\nNothing was renamed. Pass --apply to commit.");
        return Ok(());
    }

    if !plan.summary.can_apply() {
        bail!(
            "{} row(s) block Apply. Resolve them, or choose --on-conflict skip or suffix.",
            plan.summary.blocking()
        );
    }

    let journal_dir = journal::default_dir()?;
    let report = execute(
        &plan.rows,
        &plan.profile,
        &args.paths,
        Some(preset.name.clone()),
        &ExecuteOptions {
            conflict,
            paranoid: args.paranoid,
            dry_run: false,
            journal_dir: Some(&journal_dir),
        },
    )?;

    println!("\nRenamed {} file(s).", report.renamed);
    if let Some(id) = &report.journal_id {
        println!("Undo with: zrename undo {id}");
    }
    report_problems(&report);
    Ok(())
}

fn report_problems(report: &zrename_core::ExecuteReport) {
    for (path, why) in &report.skipped {
        println!("  skipped {}: {why}", path.display());
    }
    for (path, why) in &report.failed {
        eprintln!("  FAILED  {}: {why}", path.display());
    }
    for path in &report.stranded {
        eprintln!(
            "  STRANDED at a temporary name, needs attention: {}",
            path.display()
        );
    }
}

fn print_plan(plan: &zrename_core::Plan) {
    let width = plan
        .rows
        .iter()
        .map(|r| r.from_name().chars().count())
        .max()
        .unwrap_or(0)
        .min(60);
    for row in &plan.rows {
        let label = export::status_label(&row.status);
        let mark = if row.status.is_actionable() {
            "->"
        } else if row.status.is_blocking() {
            "!!"
        } else {
            "  "
        };
        println!(
            "{:<width$}  {mark}  {:<40}  {}",
            truncate(&row.from_name(), width),
            truncate(&row.to_name(), 40),
            label,
            width = width
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let keep: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{keep}\u{2026}")
}

fn load_preset(spec: &str) -> Result<Preset> {
    let as_path = PathBuf::from(spec);
    if as_path.extension().and_then(|e| e.to_str()) == Some("toml") {
        return Preset::load(&as_path).map_err(Into::into);
    }

    let dir = presets::default_dir()?;
    let available = presets::list(&dir);
    if let Some(p) = available.iter().find(|p| p.name.eq_ignore_ascii_case(spec)) {
        return Ok(p.clone());
    }
    if let Some(p) = available
        .iter()
        .find(|p| presets::slug(&p.name) == presets::slug(spec))
    {
        return Ok(p.clone());
    }

    let names: Vec<String> = available.iter().map(|p| p.name.clone()).collect();
    if names.is_empty() {
        bail!(
            "no presets found in {}. Run `zrename init-presets` first.",
            dir.display()
        );
    }
    bail!("no preset called `{spec}`. Available: {}", names.join(", "))
}

fn list_presets() -> Result<()> {
    let dir = presets::default_dir()?;
    let all = presets::list(&dir);
    if all.is_empty() {
        println!(
            "No presets in {}. Run `zrename init-presets`.",
            dir.display()
        );
        return Ok(());
    }
    println!("{}\n", dir.display());
    for p in all {
        println!("  {}  ({} rules)", p.name, p.rules.len());
        if let Some(d) = &p.description {
            println!("      {d}");
        }
    }
    Ok(())
}

fn init_presets() -> Result<()> {
    let dir = presets::default_dir()?;
    let n = presets::install_starters(&dir)?;
    println!("Wrote {n} preset(s) to {}", dir.display());
    Ok(())
}

fn history(limit: usize) -> Result<()> {
    let dir = journal::default_dir()?;
    let all = journal::list(&dir)?;
    if all.is_empty() {
        println!("No batches recorded yet.");
        return Ok(());
    }
    for s in all.iter().take(limit) {
        let preset = s.preset.clone().unwrap_or_else(|| "-".into());
        println!("{}  {:>6} file(s)  {}", s.id, s.count, preset);
    }
    Ok(())
}

fn undo(id: Option<String>, force: bool, dry_run: bool) -> Result<()> {
    let dir = journal::default_dir()?;
    let all = journal::list(&dir)?;

    let chosen = match &id {
        Some(want) => all.iter().find(|s| &s.id == want),
        None => all.first(),
    };
    let Some(chosen) = chosen else {
        bail!(match id {
            Some(want) => format!("no batch with id `{want}`"),
            None => "no batches to undo".to_string(),
        })
    };

    let j = Journal::load(&chosen.path)?;
    let report = journal::undo(&j, &UndoOptions { force, dry_run });

    println!(
        "Reverted {} of {} file(s).",
        report.reverted,
        j.entries.len()
    );
    for s in &report.skipped {
        println!("  left alone: {} ({})", s.entry.to.display(), s.detail);
    }
    for (path, why) in &report.failed {
        eprintln!("  FAILED {}: {why}", path.display());
    }
    if !report.skipped.is_empty() && !force {
        println!(
            "\nSome files changed since the rename. Re-run with --force to revert them anyway."
        );
    }
    Ok(())
}
