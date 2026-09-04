use crate::error::Result;
use crate::execute::{self, ConflictPolicy};
use crate::model::{FileEntry, FsProfile, MissingToken, Plan, PlanRow, RowStatus, RuleSpec};
use crate::rules::{self, CompiledRule, RenameCtx};
use crate::tokens::MetadataProvider;
use crate::validate::{self, ValidationInput};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanOptions {
    pub placeholder: String,
    pub profile: FsProfile,
    pub long_paths_enabled: bool,

    pub windows_paths: bool,

    #[serde(default)]
    pub on_missing_token: MissingToken,

    pub conflict: ConflictPolicy,
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            placeholder: "_".into(),
            profile: FsProfile::unknown(),
            long_paths_enabled: false,
            windows_paths: cfg!(windows),
            on_missing_token: MissingToken::default(),
            conflict: ConflictPolicy::default(),
        }
    }
}

impl PlanOptions {
    pub fn for_path(path: &Path) -> Self {
        Self {
            profile: crate::fsinfo::detect_profile(path),
            ..Self::default()
        }
    }
}

pub fn compute_targets(
    entries: &[FileEntry],
    specs: &[RuleSpec],
    meta: &dyn MetadataProvider,
    opts: &PlanOptions,
) -> Result<Vec<PlanRow>> {
    let compiled = rules::compile_stack(specs)?;
    let ordinals = assign_all_ordinals(entries, &compiled);
    let total = entries.len();

    entries
        .par_iter()
        .enumerate()
        .map(|(i, entry)| {
            let mut ctx = RenameCtx::new(entry, i, total, &opts.profile, meta, &opts.placeholder);
            ctx.on_missing = opts.on_missing_token;

            for (pos, rule) in compiled.iter().enumerate() {
                ctx.counter = match &ordinals[pos] {
                    Some(v) => v[i],
                    None => i as i64 + 1,
                };
                rule.apply(&mut ctx)?;
                if ctx.skip.is_some() {
                    break;
                }
            }

            let status = match &ctx.skip {
                Some(reason) => RowStatus::Skipped {
                    reason: reason.clone(),
                },
                None => RowStatus::Ok,
            };
            let to = target_path(entry, &ctx);

            Ok(PlanRow {
                index: i,
                from: entry.path.clone(),
                to,
                status,
                is_dir: entry.is_dir,
                is_symlink: entry.is_symlink,
                case_only: false,
            })
        })
        .collect()
}

fn assign_all_ordinals(
    entries: &[FileEntry],
    compiled: &[Box<dyn CompiledRule>],
) -> Vec<Option<Vec<i64>>> {
    compiled
        .iter()
        .map(|rule| {
            rule.needs_ordinals()
                .map(|p| rules::number::assign_ordinals(entries, p))
        })
        .collect()
}

fn target_path(entry: &FileEntry, ctx: &RenameCtx) -> PathBuf {
    let mut path = entry.parent();
    if let Some(sub) = &ctx.subdir {
        path.push(sub);
    }
    path.push(ctx.file_name());
    path
}

pub fn finish_plan(mut rows: Vec<PlanRow>, existing: &HashSet<String>, opts: &PlanOptions) -> Plan {
    let input = ValidationInput {
        profile: &opts.profile,
        existing,
        long_paths_enabled: opts.long_paths_enabled,
        windows_paths: opts.windows_paths,
    };

    let mut summary = validate::validate(&mut rows, &input);
    if summary.collisions > 0 && opts.conflict != ConflictPolicy::Stop {
        summary = resolve_conflicts(&mut rows, &input, opts.conflict);
    }

    Plan {
        rows,
        summary,
        profile: opts.profile.clone(),
    }
}

fn resolve_conflicts(
    rows: &mut [PlanRow],
    input: &ValidationInput,
    policy: ConflictPolicy,
) -> crate::model::PlanSummary {
    match policy {
        ConflictPolicy::Stop => return validate::summarise(rows),

        ConflictPolicy::Overwrite => {
            for row in rows.iter_mut() {
                if let RowStatus::Collision {
                    with,
                    existing: true,
                } = &row.status
                {
                    if with.is_empty() {
                        row.status = RowStatus::Ok;
                    }
                }
            }
            return validate::summarise(rows);
        }

        ConflictPolicy::Skip => {
            for i in losers(rows) {
                rows[i].status = RowStatus::Skipped {
                    reason: "target already taken".into(),
                };
            }
        }

        ConflictPolicy::Suffix => {
            let mut taken: HashSet<String> = input.existing.clone();
            for row in rows.iter() {
                if row.status.is_actionable() {
                    taken.insert(validate::fold_path(input.profile, &row.to));
                }
            }
            for i in losers(rows) {
                let fold = |p: &std::path::Path| validate::fold_path(input.profile, p);
                let Some(free) = execute::suffixed(&rows[i].to, |p| taken.contains(&fold(p)))
                else {
                    rows[i].status = RowStatus::Skipped {
                        reason: "no free suffix".into(),
                    };
                    continue;
                };
                taken.insert(fold(&free));
                rows[i].to = free;
                rows[i].status = RowStatus::Ok;
            }
        }
    }

    validate::validate(rows, input)
}

fn losers(rows: &[PlanRow]) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if let RowStatus::Collision { with, existing } = &row.status {
            let beaten = *existing || with.iter().any(|&j| j < i);
            if beaten {
                out.push(i);
            }
        }
    }
    out
}

pub fn build_plan(
    entries: &[FileEntry],
    specs: &[RuleSpec],
    meta: &dyn MetadataProvider,
    opts: &PlanOptions,
) -> Result<Plan> {
    let rows = compute_targets(entries, specs, meta, opts)?;
    let existing = gather_existing(&rows, &opts.profile);
    Ok(finish_plan(rows, &existing, opts))
}

pub fn gather_existing(rows: &[PlanRow], profile: &FsProfile) -> HashSet<String> {
    let mut dirs: HashSet<PathBuf> = HashSet::new();
    for row in rows {
        if let Some(p) = row.to.parent() {
            dirs.insert(p.to_path_buf());
        }
    }

    let mut out = HashSet::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in rd.flatten() {
            out.insert(validate::fold_path(profile, &item.path()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CaseStyle, ExtMode, InsertAt, MissingToken, RuleKind, Scope, SortKey};
    use crate::tokens::NullProvider;
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime};

    fn entry(path: &str, size: u64) -> FileEntry {
        let p = PathBuf::from(path);
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let (stem, ext) = FileEntry::split_name(&name);
        FileEntry {
            path: p,
            stem,
            ext,
            is_dir: false,
            is_symlink: false,
            size,
            mtime: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
            created: None,
            depth: 1,
        }
    }

    fn opts() -> PlanOptions {
        PlanOptions {
            profile: FsProfile::ext4(),
            ..Default::default()
        }
    }

    fn plan(entries: &[FileEntry], specs: &[RuleSpec], opts: &PlanOptions) -> Plan {
        let rows = compute_targets(entries, specs, &NullProvider, opts).unwrap();
        finish_plan(rows, &HashSet::new(), opts)
    }

    fn names(p: &Plan) -> Vec<String> {
        p.rows.iter().map(|r| r.to_name()).collect()
    }

    struct FakeMeta(HashMap<String, String>);

    impl MetadataProvider for FakeMeta {
        fn resolve(
            &self,
            entry: &FileEntry,
            ns: &str,
            key: &str,
            fmt: Option<&str>,
        ) -> Option<String> {
            let raw = self.0.get(&format!("{}|{ns}:{key}", entry.file_name()))?;
            match fmt {
                Some(f) => match chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
                    Ok(date) => Some(date.format(f).to_string()),
                    Err(_) => Some(raw.clone()),
                },
                None => Some(raw.clone()),
            }
        }
    }

    #[test]
    fn an_empty_stack_changes_nothing() {
        let files = [entry("/a/one.txt", 1), entry("/a/two.txt", 2)];
        let p = plan(&files, &[], &opts());
        assert_eq!(p.summary.unchanged, 2);
        assert_eq!(p.summary.changed, 0);
        assert!(!p.summary.can_apply());
    }

    #[test]
    fn rules_apply_in_stack_order() {
        let files = [entry("/a/IMG_1.JPG", 1)];
        let lower = RuleSpec::new(RuleKind::Case {
            style: CaseStyle::Lower,
        });
        let prefix = RuleSpec::new(RuleKind::Insert {
            text: "X".into(),
            at: InsertAt::Prefix,
        });

        let a = plan(&files, &[lower.clone(), prefix.clone()], &opts());
        assert_eq!(names(&a), vec!["Ximg_1.JPG"]);

        let b = plan(&files, &[prefix, lower], &opts());
        assert_eq!(names(&b), vec!["ximg_1.JPG"]);
    }

    #[test]
    fn a_disabled_rule_is_skipped() {
        let files = [entry("/a/IMG_1.JPG", 1)];
        let mut lower = RuleSpec::new(RuleKind::Case {
            style: CaseStyle::Lower,
        });
        lower.enabled = false;
        let p = plan(&files, &[lower], &opts());
        assert_eq!(p.summary.unchanged, 1);
    }

    #[test]
    fn the_photos_preset_from_the_spec_produces_dated_sequential_names() {
        let files = [
            entry("/a/IMG_4821.JPG", 1),
            entry("/a/IMG_4822.JPG", 2),
            entry("/a/IMG_4823.jpg", 3),
        ];
        let mut m = HashMap::new();
        for n in ["IMG_4821.JPG", "IMG_4822.JPG", "IMG_4823.jpg"] {
            m.insert(
                format!("{n}|exif:DateTimeOriginal"),
                "2026-08-14".to_string(),
            );
        }

        let specs = vec![
            RuleSpec::new(RuleKind::Template {
                template: "%exif:DateTimeOriginal:%Y-%m-%d%".into(),
            }),
            RuleSpec::new(RuleKind::Number {
                start: 1,
                step: 1,
                pad: 2,
                reset_per_folder: true,
                sort: SortKey::Natural,
                descending: false,
                at: InsertAt::Suffix,
            }),
            RuleSpec::new(RuleKind::Extension {
                mode: ExtMode::Lower,
            }),
        ];

        let o = opts();
        let rows = compute_targets(&files, &specs, &FakeMeta(m), &o).unwrap();
        let p = finish_plan(rows, &HashSet::new(), &o);

        assert_eq!(
            names(&p),
            vec!["2026-08-1401.jpg", "2026-08-1402.jpg", "2026-08-1403.jpg"]
        );
        assert_eq!(p.summary.collisions, 0);
        assert_eq!(p.summary.changed, 3);
        assert!(p.summary.can_apply());
    }

    #[test]
    fn a_separator_between_date_and_counter_reads_as_the_spec_shows() {
        let files = [entry("/a/IMG_4821.JPG", 1), entry("/a/IMG_4822.JPG", 2)];
        let mut m = HashMap::new();
        for n in ["IMG_4821.JPG", "IMG_4822.JPG"] {
            m.insert(
                format!("{n}|exif:DateTimeOriginal"),
                "2026-08-14".to_string(),
            );
        }
        let specs = vec![
            RuleSpec::new(RuleKind::Template {
                template: "%exif:DateTimeOriginal:%Y-%m-%d%_%counter:2%".into(),
            }),
            RuleSpec::new(RuleKind::Extension {
                mode: ExtMode::Lower,
            }),
        ];
        let o = opts();
        let rows = compute_targets(&files, &specs, &FakeMeta(m), &o).unwrap();
        let p = finish_plan(rows, &HashSet::new(), &o);
        assert_eq!(names(&p), vec!["2026-08-14_01.jpg", "2026-08-14_02.jpg"]);
    }

    #[test]
    fn a_missing_exif_tag_yields_a_placeholder_rather_than_failing() {
        let files = [entry("/a/IMG_4821.JPG", 1)];
        let specs = vec![RuleSpec::new(RuleKind::Template {
            template: "%exif:DateTimeOriginal:%Y-%m-%d%".into(),
        })];
        let o = PlanOptions {
            placeholder: "nodate".into(),
            ..opts()
        };
        let p = plan(&files, &specs, &o);
        assert_eq!(names(&p), vec!["nodate.JPG"]);
        assert_eq!(p.summary.changed, 1);
    }

    #[test]
    fn a_file_can_be_left_alone_when_a_token_has_no_value() {
        let files = [entry("/a/IMG_4821.JPG", 1), entry("/a/DSC_0007.jpg", 2)];
        let mut m = HashMap::new();
        m.insert(
            "IMG_4821.JPG|exif:DateTimeOriginal".to_string(),
            "2026-08-14".to_string(),
        );

        let specs = vec![RuleSpec::new(RuleKind::Template {
            template: "%exif:DateTimeOriginal:%Y-%m-%d%".into(),
        })];

        let o = PlanOptions {
            placeholder: "_".into(),
            ..opts()
        };
        let rows = compute_targets(&files, &specs, &FakeMeta(m.clone()), &o).unwrap();
        let p = finish_plan(rows, &HashSet::new(), &o);
        assert_eq!(names(&p), vec!["2026-08-14.JPG", "_.jpg"]);
        assert_eq!(p.summary.changed, 2);

        let o = PlanOptions {
            on_missing_token: MissingToken::Skip,
            ..o
        };
        let rows = compute_targets(&files, &specs, &FakeMeta(m), &o).unwrap();
        let p = finish_plan(rows, &HashSet::new(), &o);
        assert_eq!(p.summary.changed, 1);
        assert_eq!(p.summary.skipped, 1);
        assert!(matches!(p.rows[1].status, RowStatus::Skipped { .. }));
    }

    #[test]
    fn the_collision_from_the_spec_mockup_is_flagged() {
        let files = [entry("/a/IMG_4822.JPG", 1), entry("/a/IMG_4823.jpg", 2)];
        let specs = vec![
            RuleSpec::new(RuleKind::Template {
                template: "2026-08-14_02".into(),
            }),
            RuleSpec::new(RuleKind::Extension {
                mode: ExtMode::Lower,
            }),
        ];
        let o = opts();
        let p = plan(&files, &specs, &o);
        assert_eq!(p.summary.collisions, 2);
        assert!(
            !p.summary.can_apply(),
            "\u{a7}6: Apply stays disabled while a collision stands"
        );

        let bare = vec![RuleSpec::new(RuleKind::Template {
            template: "2026-08-14_02".into(),
        })];
        assert_eq!(plan(&files, &bare, &o).summary.collisions, 0);
        assert_eq!(
            plan(
                &files,
                &bare,
                &PlanOptions {
                    profile: FsProfile::ntfs(),
                    ..Default::default()
                }
            )
            .summary
            .collisions,
            2,
            "on NTFS the case difference does not save them"
        );
    }

    #[test]
    fn numbering_resets_per_folder_across_a_recursive_selection() {
        let files = [
            entry("/a/one.jpg", 1),
            entry("/a/two.jpg", 2),
            entry("/b/one.jpg", 3),
            entry("/b/two.jpg", 4),
        ];
        let specs = vec![
            RuleSpec::new(RuleKind::Template {
                template: "shot".into(),
            }),
            RuleSpec::new(RuleKind::Number {
                start: 1,
                step: 1,
                pad: 2,
                reset_per_folder: true,
                sort: SortKey::Name,
                descending: false,
                at: InsertAt::Suffix,
            }),
        ];
        let p = plan(&files, &specs, &opts());
        assert_eq!(
            names(&p),
            vec!["shot01.jpg", "shot02.jpg", "shot01.jpg", "shot02.jpg"]
        );
        assert_eq!(
            p.summary.collisions, 0,
            "same names in different folders do not collide"
        );
    }

    #[test]
    fn two_numbering_rules_keep_independent_sequences() {
        let files = [entry("/a/a.txt", 1), entry("/a/b.txt", 2)];
        let specs = vec![
            RuleSpec::new(RuleKind::Template {
                template: "f".into(),
            }),
            RuleSpec::new(RuleKind::Number {
                start: 1,
                step: 1,
                pad: 1,
                reset_per_folder: false,
                sort: SortKey::Name,
                descending: false,
                at: InsertAt::Suffix,
            }),
            RuleSpec::new(RuleKind::Number {
                start: 100,
                step: 10,
                pad: 3,
                reset_per_folder: false,
                sort: SortKey::Name,
                descending: true,
                at: InsertAt::Suffix,
            }),
        ];
        let p = plan(&files, &specs, &opts());
        assert_eq!(names(&p), vec!["f1110.txt", "f2100.txt"]);
    }

    #[test]
    fn planning_is_deterministic_however_many_times_it_runs() {
        let files: Vec<FileEntry> = (0..500)
            .map(|i| entry(&format!("/a/f{i}.txt"), i as u64))
            .collect();
        let specs = vec![
            RuleSpec::new(RuleKind::Case {
                style: CaseStyle::Upper,
            }),
            RuleSpec::new(RuleKind::Number {
                start: 1,
                step: 1,
                pad: 4,
                reset_per_folder: false,
                sort: SortKey::Natural,
                descending: false,
                at: InsertAt::Prefix,
            }),
        ];
        let o = opts();
        let first = names(&plan(&files, &specs, &o));
        for _ in 0..4 {
            assert_eq!(
                names(&plan(&files, &specs, &o)),
                first,
                "parallel planning must be stable"
            );
        }
        assert_eq!(first.len(), 500);
    }

    #[test]
    fn an_invalid_regex_stops_the_plan_instead_of_producing_wrong_names() {
        let files = [entry("/a/a.txt", 1)];
        let specs = vec![RuleSpec::new(RuleKind::Replace {
            find: "(unclosed".into(),
            with: "x".into(),
            regex: true,
            case_sensitive: true,
            all: true,
        })];
        assert!(compute_targets(&files, &specs, &NullProvider, &opts()).is_err());
    }

    #[test]
    fn move_into_files_results_under_a_token_derived_subfolder() {
        let files = [entry("/a/IMG_1.JPG", 1)];
        let mut m = HashMap::new();
        m.insert(
            "IMG_1.JPG|exif:DateTimeOriginal".to_string(),
            "2026-08-14".to_string(),
        );
        let specs = vec![RuleSpec::new(RuleKind::MoveInto {
            template: "%exif:DateTimeOriginal:%Y%/%exif:DateTimeOriginal:%m%".into(),
        })];
        let o = opts();
        let rows = compute_targets(&files, &specs, &FakeMeta(m), &o).unwrap();
        assert_eq!(rows[0].to, PathBuf::from("/a/2026/08/IMG_1.JPG"));
    }

    #[test]
    fn a_case_only_batch_is_planned_as_actionable_on_windows() {
        let files = [entry("/a/photo.JPG", 1)];
        let specs = vec![RuleSpec::new(RuleKind::Extension {
            mode: ExtMode::Lower,
        })];
        let o = PlanOptions {
            profile: FsProfile::ntfs(),
            ..Default::default()
        };
        let rows = compute_targets(&files, &specs, &NullProvider, &o).unwrap();
        let existing: HashSet<String> = ["/a/photo.jpg".to_string()].into_iter().collect();
        let p = finish_plan(rows, &existing, &o);
        assert_eq!(p.summary.changed, 1);
        assert!(
            p.rows[0].case_only,
            "must be routed through a two-phase rename"
        );
        assert!(p.summary.can_apply());
    }

    #[test]
    fn scoping_a_rule_to_the_extension_leaves_the_stem_alone() {
        let files = [entry("/a/Report.TXT", 1)];
        let specs = vec![RuleSpec::new(RuleKind::Case {
            style: CaseStyle::Lower,
        })
        .with_scope(Scope::EXT)];
        let p = plan(&files, &specs, &opts());
        assert_eq!(names(&p), vec!["Report.txt"]);
    }

    #[test]
    fn the_sanitise_for_usb_preset_cleans_a_name_for_fat32() {
        let files = [entry("/a/Urlaub 2024: M\u{fc}nchen <best>.JPG", 1)];
        let specs = vec![RuleSpec::new(RuleKind::Sanitise {
            illegal: true,
            collapse_spaces: true,
            transliterate: true,
            replacement: "_".into(),
            trim_dots_spaces: true,
        })];
        let o = PlanOptions {
            profile: FsProfile::fat32(),
            ..Default::default()
        };
        let p = plan(&files, &specs, &o);
        assert_eq!(names(&p), vec!["Urlaub 2024_ Munchen _best_.JPG"]);
        assert_eq!(p.summary.invalid, 0);
        assert!(p.summary.can_apply());
    }
}
