use crate::dto::*;
use crate::state::AppState;
use std::path::PathBuf;
use tauri::State;
use zrename_core::execute::{execute, ConflictPolicy, ExecuteOptions};
use zrename_core::journal::{self, Journal, UndoOptions};
use zrename_core::model::{FsProfile, RuleSpec};
use zrename_core::plan::{self, build_plan, PlanOptions};
use zrename_core::presets::{self, Preset};
use zrename_core::scan::{scan, ScanOptions};
use zrename_core::{dupes, export};

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub fn startup_args() -> StartupArgs {
    parse_args(std::env::args().skip(1).collect())
}

pub fn parse_args(args: Vec<String>) -> StartupArgs {
    let mut out = StartupArgs::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--preset" | "-p" => out.preset = it.next(),
            _ if a.starts_with("--preset=") => {
                out.preset = Some(a["--preset=".len()..].to_string())
            }
            _ if a.starts_with('-') => {}
            _ if std::path::Path::new(&a).exists() => out.paths.push(a),
            _ => {}
        }
    }
    out
}

#[tauri::command]
pub fn capabilities(state: State<AppState>) -> Res<Capabilities> {
    Ok(Capabilities {
        ffprobe: state.meta.has_ffprobe(),
        preset_dir: presets::default_dir()
            .map_err(err)?
            .to_string_lossy()
            .into_owned(),
        journal_dir: journal::default_dir()
            .map_err(err)?
            .to_string_lossy()
            .into_owned(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[tauri::command]
pub fn scan_paths(
    state: State<AppState>,
    paths: Vec<String>,
    options: ScanOptions,
) -> Res<ScanResult> {
    let roots: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let entries = scan(&roots, &options).map_err(err)?;

    let probe = roots.first().cloned().unwrap_or_default();
    let profile = zrename_core::fsinfo::detect_profile(&probe);

    let files = entries.iter().filter(|e| !e.is_dir).count();
    let result = ScanResult {
        total: entries.len(),
        files,
        folders: entries.len() - files,
        roots: paths,
        fs_name: profile.name.clone(),
        case_insensitive: profile.case_insensitive,
        max_path: profile.max_path,
        needs_sanitising: !profile.reserved_stems.is_empty(),
    };

    let mut s = state.session.lock().map_err(err)?;
    s.roots = roots;
    s.entries = entries;
    s.scan_opts = options;
    s.plan_opts = Some(PlanOptions {
        conflict: s.options().conflict,
        ..PlanOptions {
            profile,
            ..PlanOptions::default()
        }
    });
    s.plan = None;
    s.excluded.clear();
    s.invalidate_view();
    Ok(result)
}

#[tauri::command]
pub fn set_rules(state: State<AppState>, rules: Vec<RuleSpec>) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    s.rules = rules;
    replan(&mut s, &state.meta)
}

#[tauri::command]
pub fn set_conflict_policy(state: State<AppState>, policy: ConflictPolicy) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    let mut opts = s.options();
    opts.conflict = policy;
    s.plan_opts = Some(opts);
    replan(&mut s, &state.meta)
}

#[tauri::command]
pub fn set_placeholder(state: State<AppState>, placeholder: String) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    let mut opts = s.options();
    opts.placeholder = placeholder;
    s.plan_opts = Some(opts);
    replan(&mut s, &state.meta)
}

fn replan(
    s: &mut crate::state::Session,
    meta: &zrename_core::metadata::LazyMetadata,
) -> Res<SummaryDto> {
    if s.entries.is_empty() {
        s.plan = None;
        s.invalidate_view();
        return Ok(SummaryDto::empty());
    }
    let opts = s.options();

    let mut rows = plan::compute_targets(&s.entries, &s.rules, meta, &opts).map_err(err)?;
    for row in rows.iter_mut() {
        if s.excluded.contains(&row.index) {
            row.status = zrename_core::RowStatus::Skipped {
                reason: "unticked".into(),
            };
        }
    }

    let existing = plan::gather_existing(&rows, &opts.profile);
    let plan = plan::finish_plan(rows, &existing, &opts);

    let dto = SummaryDto::of(&plan);
    s.plan = Some(plan);
    s.invalidate_view();
    Ok(dto)
}

#[tauri::command]
pub fn set_row_excluded(state: State<AppState>, index: usize, excluded: bool) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    if excluded {
        s.excluded.insert(index);
    } else {
        s.excluded.remove(&index);
    }
    replan(&mut s, &state.meta)
}

#[tauri::command]
pub fn exclude_rows(
    state: State<AppState>,
    indices: Vec<usize>,
    excluded: bool,
) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    for i in indices {
        if excluded {
            s.excluded.insert(i);
        } else {
            s.excluded.remove(&i);
        }
    }
    replan(&mut s, &state.meta)
}

#[tauri::command]
pub fn clear_exclusions(state: State<AppState>) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    s.excluded.clear();
    replan(&mut s, &state.meta)
}

#[tauri::command]
pub fn set_missing_token(
    state: State<AppState>,
    policy: zrename_core::MissingToken,
) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    let mut opts = s.options();
    opts.on_missing_token = policy;
    s.plan_opts = Some(opts);
    replan(&mut s, &state.meta)
}

#[tauri::command]
pub fn set_long_paths(state: State<AppState>, enabled: bool) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    let mut opts = s.options();
    opts.long_paths_enabled = enabled;
    s.plan_opts = Some(opts);
    replan(&mut s, &state.meta)
}

#[tauri::command]
pub fn get_rows(state: State<AppState>, query: RowQuery) -> Res<RowPage> {
    let mut s = state.session.lock().map_err(err)?;
    let view: Vec<usize> = s.view(&query).to_vec();
    let total = view.len();
    let excluded = s.excluded.clone();

    let Some(plan) = &s.plan else {
        return Ok(RowPage {
            rows: Vec::new(),
            total: 0,
        });
    };

    let end = (query.offset + query.limit).min(total);
    let rows = if query.offset >= total {
        Vec::new()
    } else {
        view[query.offset..end]
            .iter()
            .map(|&i| {
                let row = &plan.rows[i];
                RowDto::of(row, excluded.contains(&row.index))
            })
            .collect()
    };
    Ok(RowPage { rows, total })
}

#[tauri::command]
pub fn rescan(state: State<AppState>) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    if s.roots.is_empty() {
        return Ok(SummaryDto::empty());
    }
    let roots = s.roots.clone();
    let opts = s.scan_opts.clone();
    s.entries = scan(&roots, &opts).map_err(err)?;
    state.meta.clear();
    replan(&mut s, &state.meta)
}

#[tauri::command]
pub fn set_scan_options(state: State<AppState>, options: ScanOptions) -> Res<SummaryDto> {
    let mut s = state.session.lock().map_err(err)?;
    s.scan_opts = options;
    if s.roots.is_empty() {
        return Ok(SummaryDto::empty());
    }
    let roots = s.roots.clone();
    let opts = s.scan_opts.clone();
    s.entries = scan(&roots, &opts).map_err(err)?;
    replan(&mut s, &state.meta)
}

#[tauri::command]
pub async fn apply(
    state: State<'_, AppState>,
    preset: Option<String>,
    paranoid: bool,
) -> Res<ApplyResult> {
    let (rows, profile, roots, conflict) = {
        let s = state.session.lock().map_err(err)?;
        let Some(plan) = &s.plan else {
            return Err("nothing to apply".into());
        };
        if !plan.summary.can_apply() {
            return Err(format!(
                "{} row(s) still need attention",
                plan.summary.blocking()
            ));
        }
        (
            plan.rows.clone(),
            plan.profile.clone(),
            s.roots.clone(),
            s.options().conflict,
        )
    };

    let journal_dir = journal::default_dir().map_err(err)?;
    let report = tauri::async_runtime::spawn_blocking(move || {
        execute(
            &rows,
            &profile,
            &roots,
            preset,
            &ExecuteOptions {
                conflict,
                paranoid,
                dry_run: false,
                journal_dir: Some(&journal_dir),
            },
        )
    })
    .await
    .map_err(err)?
    .map_err(err)?;

    Ok(ApplyResult {
        renamed: report.renamed,
        two_phase: report.two_phase,
        skipped: report
            .skipped
            .iter()
            .map(|(p, w)| [p.to_string_lossy().into_owned(), w.clone()])
            .collect(),
        failed: report
            .failed
            .iter()
            .map(|(p, w)| [p.to_string_lossy().into_owned(), w.clone()])
            .collect(),
        stranded: report
            .stranded
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        journal_id: report.journal_id.clone(),
        clean: report.is_clean(),
    })
}

#[tauri::command]
pub fn list_history() -> Res<Vec<HistoryEntry>> {
    let dir = journal::default_dir().map_err(err)?;
    Ok(journal::list(&dir)
        .map_err(err)?
        .into_iter()
        .map(|s| HistoryEntry {
            id: s.id,
            created: s.created,
            count: s.count,
            preset: s.preset,
            roots: s
                .roots
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        })
        .collect())
}

#[tauri::command]
pub async fn undo_batch(id: Option<String>, force: bool) -> Res<UndoResult> {
    let dir = journal::default_dir().map_err(err)?;
    let all = journal::list(&dir).map_err(err)?;
    let chosen = match &id {
        Some(want) => all.iter().find(|s| &s.id == want),
        None => all.first(),
    };
    let Some(chosen) = chosen else {
        return Err("no batch to undo".into());
    };

    let path = chosen.path.clone();
    let (report, total) = tauri::async_runtime::spawn_blocking(move || {
        let j = Journal::load(&path)?;
        let r = journal::undo(
            &j,
            &UndoOptions {
                force,
                dry_run: false,
            },
        );
        Ok::<_, zrename_core::CoreError>((r, j.entries.len()))
    })
    .await
    .map_err(err)?
    .map_err(err)?;

    Ok(UndoResult {
        reverted: report.reverted,
        total,
        skipped: report
            .skipped
            .iter()
            .map(|s| SkipDto {
                name: s
                    .entry
                    .to
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                kind: format!("{:?}", s.kind).to_lowercase(),
                detail: s.detail.clone(),
            })
            .collect(),
        failed: report
            .failed
            .iter()
            .map(|(p, w)| [p.to_string_lossy().into_owned(), w.clone()])
            .collect(),
        clean: report.is_clean(),
    })
}

#[tauri::command]
pub fn list_presets() -> Res<Vec<Preset>> {
    let dir = presets::default_dir().map_err(err)?;
    if presets::list(&dir).is_empty() {
        let _ = presets::install_starters(&dir);
    }
    Ok(presets::list(&dir))
}

#[tauri::command]
pub fn save_preset(preset: Preset) -> Res<String> {
    let dir = presets::default_dir().map_err(err)?;
    Ok(preset
        .save(&dir)
        .map_err(err)?
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub fn delete_preset(name: String) -> Res<()> {
    let dir = presets::default_dir().map_err(err)?;
    let path = dir.join(format!("{}.toml", presets::slug(&name)));
    std::fs::remove_file(&path).map_err(err)
}

#[tauri::command]
pub fn import_preset(path: String) -> Res<Preset> {
    let p = Preset::load(std::path::Path::new(&path)).map_err(err)?;
    let dir = presets::default_dir().map_err(err)?;
    p.save(&dir).map_err(err)?;
    Ok(p)
}

#[tauri::command]
pub fn export_preset(preset: Preset, path: String) -> Res<()> {
    std::fs::write(&path, preset.to_toml().map_err(err)?).map_err(err)
}

#[tauri::command]
pub fn regex_test(
    pattern: String,
    sample: String,
    replacement: String,
    case_sensitive: bool,
) -> Res<RegexTest> {
    let source = if case_sensitive {
        pattern.clone()
    } else {
        format!("(?i){pattern}")
    };
    let re = match regex::Regex::new(&source) {
        Ok(re) => re,
        Err(e) => {
            return Ok(RegexTest {
                valid: false,
                error: Some(first_line(&e.to_string())),
                matched: false,
                groups: Vec::new(),
                preview: None,
            })
        }
    };

    let caps = re.captures(&sample);
    let groups = caps
        .as_ref()
        .map(|c| {
            c.iter()
                .skip(1)
                .map(|m| m.map(|m| m.as_str().to_string()).unwrap_or_default())
                .collect()
        })
        .unwrap_or_default();

    Ok(RegexTest {
        valid: true,
        error: None,
        matched: caps.is_some(),
        groups,
        preview: Some(re.replace_all(&sample, replacement.as_str()).into_owned()),
    })
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(s)
        .trim()
        .to_string()
}

#[tauri::command]
pub fn export_plan(state: State<AppState>, format: String) -> Res<String> {
    let s = state.session.lock().map_err(err)?;
    let Some(plan) = &s.plan else {
        return Err("nothing planned yet".into());
    };
    match format.as_str() {
        "csv" => export::to_csv(plan).map_err(err),
        "markdown" | "md" => Ok(export::to_markdown(plan)),
        other => Err(format!("unknown export format `{other}`")),
    }
}

#[tauri::command]
pub fn find_dupes(state: State<AppState>) -> Res<Vec<DupeGroupDto>> {
    let s = state.session.lock().map_err(err)?;
    Ok(dupes::find_duplicates(&s.entries)
        .into_iter()
        .map(|g| DupeGroupDto {
            hash: g.hash[..12.min(g.hash.len())].to_string(),
            size: g.size,
            names: g
                .members
                .iter()
                .map(|&i| s.entries[i].file_name())
                .collect(),
            paths: g
                .members
                .iter()
                .map(|&i| s.entries[i].path.to_string_lossy().into_owned())
                .collect(),
            indices: g.members.clone(),
        })
        .collect())
}

#[tauri::command]
pub fn detect_fs(path: String) -> Res<FsProfile> {
    Ok(zrename_core::fsinfo::detect_profile(std::path::Path::new(
        &path,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> StartupArgs {
        parse_args(v.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn startup_arguments_split_into_paths_and_a_preset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().into_owned();

        let a = args(&[&path, "--preset", "Photos"]);
        assert_eq!(a.paths, vec![path.clone()]);
        assert_eq!(a.preset.as_deref(), Some("Photos"));

        let a = args(&["--preset=Photos", &path]);
        assert_eq!(a.preset.as_deref(), Some("Photos"));
        assert_eq!(a.paths, vec![path.clone()]);
    }

    #[test]
    fn paths_that_do_not_exist_are_dropped() {
        let a = args(&["/definitely/not/here", "--verbose"]);
        assert!(a.paths.is_empty());
        assert!(a.preset.is_none());
    }

    #[test]
    fn no_arguments_opens_empty() {
        let a = args(&[]);
        assert!(a.paths.is_empty());
        assert!(a.preset.is_none());
    }

    #[test]
    fn a_preset_flag_with_no_value_is_not_a_path() {
        let a = args(&["--preset"]);
        assert!(a.preset.is_none());
        assert!(a.paths.is_empty());
    }

    #[test]
    fn a_valid_pattern_reports_its_capture_groups() {
        let r = regex_test(
            "^IMG_(\\d+)".into(),
            "IMG_4821.JPG".into(),
            "shot-$1".into(),
            true,
        )
        .unwrap();
        assert!(r.valid);
        assert!(r.matched);
        assert_eq!(r.groups, vec!["4821".to_string()]);
        assert_eq!(r.preview.unwrap(), "shot-4821.JPG");
    }

    #[test]
    fn an_invalid_pattern_reports_one_readable_line() {
        let r = regex_test("(unclosed".into(), "x".into(), String::new(), true).unwrap();
        assert!(!r.valid);
        let e = r.error.unwrap();
        assert!(!e.is_empty());
        assert!(!e.contains('\n'), "the editor has one line of room: {e}");
    }

    #[test]
    fn case_sensitivity_is_honoured() {
        let sensitive = regex_test("img".into(), "IMG_1".into(), String::new(), true).unwrap();
        assert!(!sensitive.matched);
        let insensitive = regex_test("img".into(), "IMG_1".into(), String::new(), false).unwrap();
        assert!(insensitive.matched);
    }

    #[test]
    fn a_pattern_that_does_not_match_is_still_valid() {
        let r = regex_test("^nope".into(), "IMG_1".into(), "x".into(), true).unwrap();
        assert!(r.valid);
        assert!(!r.matched);
        assert!(r.groups.is_empty());
        assert_eq!(r.preview.unwrap(), "IMG_1");
    }
}

#[tauri::command]
pub fn watch_start(
    app: tauri::AppHandle,
    state: State<AppState>,
    preset: Option<String>,
) -> Res<()> {
    use tauri::{Emitter, Manager};

    let (roots, recursive) = {
        let s = state.session.lock().map_err(err)?;
        if s.roots.is_empty() {
            return Err("load a folder first".into());
        }
        (s.roots.clone(), s.scan_opts.recursive)
    };

    let handle = app.clone();
    let watch = zrename_core::watch::watch(
        &roots,
        recursive,
        zrename_core::watch::DEFAULT_SETTLE,
        move |paths| {
            let state = handle.state::<AppState>();
            match apply_to_arrivals(&state, &paths, preset.clone()) {
                Ok(Some(report)) => {
                    let _ = handle.emit("zrename://watch-applied", report);
                }
                Ok(None) => {}
                Err(message) => {
                    let _ = handle.emit("zrename://watch-error", message);
                }
            }
        },
    )
    .map_err(err)?;

    *state.watch.lock().map_err(err)? = Some(watch);
    Ok(())
}

#[tauri::command]
pub fn watch_stop(state: State<AppState>) -> Res<()> {
    *state.watch.lock().map_err(err)? = None;
    Ok(())
}

#[tauri::command]
pub fn watch_status(state: State<AppState>) -> Res<bool> {
    Ok(state.watch.lock().map_err(err)?.is_some())
}

fn apply_to_arrivals(
    state: &AppState,
    paths: &[PathBuf],
    preset: Option<String>,
) -> Result<Option<ApplyResult>, String> {
    let (rules, opts) = {
        let s = state.session.lock().map_err(err)?;
        if s.rules.is_empty() {
            return Ok(None);
        }
        (s.rules.clone(), s.options())
    };

    let entries = scan(paths, &ScanOptions::default()).map_err(err)?;
    if entries.is_empty() {
        return Ok(None);
    }

    let plan = build_plan(&entries, &rules, &state.meta, &opts).map_err(err)?;
    if !plan.summary.can_apply() {
        return Err(format!(
            "{} new file(s) were left alone: {}",
            entries.len(),
            zrename_core::export::summary_line(&plan)
        ));
    }

    let journal_dir = journal::default_dir().map_err(err)?;
    let report = execute(
        &plan.rows,
        &plan.profile,
        paths,
        preset,
        &ExecuteOptions {
            conflict: opts.conflict,
            paranoid: false,
            dry_run: false,
            journal_dir: Some(&journal_dir),
        },
    )
    .map_err(err)?;

    Ok(Some(ApplyResult {
        renamed: report.renamed,
        two_phase: report.two_phase,
        skipped: report
            .skipped
            .iter()
            .map(|(p, w)| [p.to_string_lossy().into_owned(), w.clone()])
            .collect(),
        failed: report
            .failed
            .iter()
            .map(|(p, w)| [p.to_string_lossy().into_owned(), w.clone()])
            .collect(),
        stranded: report
            .stranded
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        journal_id: report.journal_id.clone(),
        clean: report.is_clean(),
    }))
}
