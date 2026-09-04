use crate::model::{FsProfile, LengthUnit, PlanRow, PlanSummary, RowStatus};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct ValidationInput<'a> {
    pub profile: &'a FsProfile,

    pub existing: &'a HashSet<String>,

    pub long_paths_enabled: bool,

    pub windows_paths: bool,
}

impl ValidationInput<'_> {
    pub fn leaf(&self, path: &Path) -> String {
        let s = path.to_string_lossy();
        match s.rfind(|c| self.is_sep(c)) {
            Some(i) => s[i + 1..].to_string(),
            None => s.into_owned(),
        }
    }

    pub fn with_leaf(&self, path: &Path, leaf: &str) -> String {
        let s = path.to_string_lossy();
        match s.rfind(|c| self.is_sep(c)) {
            Some(i) => format!("{}{leaf}", &s[..=i]),
            None => leaf.to_string(),
        }
    }

    fn is_sep(&self, c: char) -> bool {
        c == '/' || (self.windows_paths && c == '\\')
    }
}

pub fn fold_path(profile: &FsProfile, path: &Path) -> String {
    let s = path.to_string_lossy();
    if profile.case_insensitive {
        s.to_lowercase()
    } else {
        s.into_owned()
    }
}

pub fn validate(rows: &mut [PlanRow], input: &ValidationInput) -> PlanSummary {
    let p = input.profile;

    for row in rows.iter_mut() {
        if matches!(row.status, RowStatus::Skipped { .. }) {
            continue;
        }
        row.case_only = false;
        row.status = judge_name(row, input);
    }

    detect_collisions(rows, input);

    let _ = p;
    summarise(rows)
}

pub fn summarise(rows: &[PlanRow]) -> PlanSummary {
    let mut s = PlanSummary {
        total: rows.len(),
        ..Default::default()
    };
    for row in rows.iter() {
        match &row.status {
            RowStatus::Ok => s.changed += 1,
            RowStatus::Unchanged => s.unchanged += 1,
            RowStatus::Collision { .. } => s.collisions += 1,
            RowStatus::Invalid { .. } => s.invalid += 1,
            RowStatus::TooLong { .. } => s.too_long += 1,
            RowStatus::ReservedName { .. } => s.reserved += 1,
            RowStatus::Skipped { .. } => s.skipped += 1,
        }
    }
    s
}

fn judge_name(row: &mut PlanRow, input: &ValidationInput) -> RowStatus {
    let p = input.profile;
    let raw = input.leaf(&row.to);

    if raw.is_empty() {
        return RowStatus::Invalid {
            reason: "the rules produced an empty name".into(),
        };
    }
    if raw == "." || raw == ".." {
        return RowStatus::Invalid {
            reason: format!("`{raw}` is not a usable file name"),
        };
    }

    if let Some(c) = raw.chars().find(|c| p.illegal_chars.contains(c)) {
        let shown = if c.is_control() {
            format!("control character U+{:04X}", c as u32)
        } else {
            format!("`{c}`")
        };
        return RowStatus::Invalid {
            reason: format!("{shown} is not allowed on {}", p.name),
        };
    }

    let effective = p.effective_name(&raw);
    if effective.is_empty() {
        return RowStatus::Invalid {
            reason: format!("{} would reduce `{raw}` to an empty name", p.name),
        };
    }

    if p.is_reserved(&effective) {
        return RowStatus::ReservedName { name: effective };
    }

    if let Some(actual) = p.max_component.exceeded_by(&effective) {
        return RowStatus::TooLong {
            limit: p.max_component.max,
            actual,
            unit: p.max_component.unit,
        };
    }

    if let Some(max) = p.max_path {
        let reachable = p.supports_long_path_prefix && input.long_paths_enabled;
        if !reachable {
            let full = row.to.to_string_lossy();
            let n = match p.max_component.unit {
                LengthUnit::Utf16Units => full.encode_utf16().count(),
                LengthUnit::Bytes => full.len(),
            };
            if n > max {
                return RowStatus::TooLong {
                    limit: max,
                    actual: n,
                    unit: p.max_component.unit,
                };
            }
        }
    }

    let from_name = input.leaf(&row.from);
    if from_name == raw && row.from == row.to {
        return RowStatus::Unchanged;
    }

    if p.case_insensitive && p.fold(&from_name) == p.fold(&raw) && from_name != raw {
        row.case_only = true;
    }

    RowStatus::Ok
}

fn detect_collisions(rows: &mut [PlanRow], input: &ValidationInput) {
    let p = input.profile;

    let mut sources: HashSet<String> = HashSet::new();
    for row in rows.iter() {
        if row.status.is_actionable() {
            sources.insert(fold_path(p, &row.from));
        }
    }

    let mut targets: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        if !row.status.is_actionable() {
            continue;
        }
        let key = fold_target(input, row);
        targets.entry(key).or_default().push(i);
    }

    let mut verdicts: Vec<(usize, RowStatus)> = Vec::new();
    for (key, idxs) in &targets {
        if idxs.len() > 1 {
            for &i in idxs {
                let others: Vec<usize> = idxs.iter().copied().filter(|&j| j != i).collect();
                verdicts.push((
                    i,
                    RowStatus::Collision {
                        with: others,
                        existing: false,
                    },
                ));
            }
            continue;
        }
        let i = idxs[0];

        if input.existing.contains(key)
            && !sources.contains(key)
            && fold_path(p, &rows[i].from) != *key
        {
            let _ = p;
            verdicts.push((
                i,
                RowStatus::Collision {
                    with: Vec::new(),
                    existing: true,
                },
            ));
        }
    }

    for (i, status) in verdicts {
        rows[i].status = status;
    }
}

fn fold_target(input: &ValidationInput, row: &PlanRow) -> String {
    let p = input.profile;
    let effective = p.effective_name(&input.leaf(&row.to));
    let full = input.with_leaf(&row.to, &effective);
    if p.case_insensitive {
        full.to_lowercase()
    } else {
        full
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn row(i: usize, from: &str, to: &str) -> PlanRow {
        PlanRow {
            index: i,
            from: PathBuf::from(from),
            to: PathBuf::from(to),
            status: RowStatus::Ok,
            is_dir: false,
            is_symlink: false,
            case_only: false,
        }
    }

    fn check(rows: &mut [PlanRow], profile: &FsProfile) -> PlanSummary {
        let existing = HashSet::new();
        validate(
            rows,
            &ValidationInput {
                profile,
                existing: &existing,
                long_paths_enabled: false,
                windows_paths: false,
            },
        )
    }

    fn check_with(rows: &mut [PlanRow], profile: &FsProfile, existing: &[&str]) -> PlanSummary {
        let set: HashSet<String> = existing
            .iter()
            .map(|p| fold_path(profile, Path::new(p)))
            .collect();
        validate(
            rows,
            &ValidationInput {
                profile,
                existing: &set,
                long_paths_enabled: false,
                windows_paths: false,
            },
        )
    }

    fn check_as_windows(rows: &mut [PlanRow], long_paths: bool) -> PlanSummary {
        let profile = FsProfile::ntfs();
        let existing = HashSet::new();
        validate(
            rows,
            &ValidationInput {
                profile: &profile,
                existing: &existing,
                long_paths_enabled: long_paths,
                windows_paths: true,
            },
        )
    }

    #[test]
    fn an_ordinary_rename_is_ok() {
        let mut rows = [row(0, "/a/IMG_1.JPG", "/a/2026-08-14_01.jpg")];
        let s = check(&mut rows, &FsProfile::ext4());
        assert_eq!(rows[0].status, RowStatus::Ok);
        assert_eq!(s.changed, 1);
        assert!(s.can_apply());
    }

    #[test]
    fn an_unchanged_row_is_not_counted_as_a_change() {
        let mut rows = [row(0, "/a/keep.txt", "/a/keep.txt")];
        let s = check(&mut rows, &FsProfile::ext4());
        assert_eq!(rows[0].status, RowStatus::Unchanged);
        assert_eq!(s.changed, 0);
        assert!(!s.can_apply());
    }

    #[test]
    fn two_rows_wanting_the_same_name_both_collide() {
        let mut rows = [
            row(0, "/a/IMG_4822.JPG", "/a/2026-08-14_02.jpg"),
            row(1, "/a/IMG_4823.jpg", "/a/2026-08-14_02.jpg"),
        ];
        let s = check(&mut rows, &FsProfile::ext4());
        assert_eq!(s.collisions, 2);
        assert!(
            !s.can_apply(),
            "apply must be blocked while a collision stands"
        );
        assert_eq!(
            rows[0].status,
            RowStatus::Collision {
                with: vec![1],
                existing: false
            }
        );
        assert_eq!(
            rows[1].status,
            RowStatus::Collision {
                with: vec![0],
                existing: false
            }
        );
    }

    #[test]
    fn names_differing_only_in_case_collide_on_windows_but_not_on_linux() {
        let mk = || {
            [
                row(0, "/a/one.txt", "/a/Report.txt"),
                row(1, "/a/two.txt", "/a/report.txt"),
            ]
        };

        let mut rows = mk();
        assert_eq!(check(&mut rows, &FsProfile::ntfs()).collisions, 2);

        let mut rows = mk();
        assert_eq!(check(&mut rows, &FsProfile::ext4()).collisions, 0);
    }

    #[test]
    fn a_case_only_rename_is_flagged_for_two_phase_not_reported_as_a_collision() {
        let mut rows = [row(0, "/a/photo.JPG", "/a/photo.jpg")];
        let s = check_with(&mut rows, &FsProfile::ntfs(), &["/a/photo.JPG"]);
        assert_eq!(
            rows[0].status,
            RowStatus::Ok,
            "this is the trap the spec calls out in \u{a7}2"
        );
        assert!(rows[0].case_only);
        assert_eq!(s.collisions, 0);
        assert!(s.can_apply());
    }

    #[test]
    fn a_case_only_rename_on_a_case_sensitive_filesystem_is_an_ordinary_rename() {
        let mut rows = [row(0, "/a/photo.JPG", "/a/photo.jpg")];
        check_with(&mut rows, &FsProfile::ext4(), &["/a/photo.JPG"]);
        assert_eq!(rows[0].status, RowStatus::Ok);
        assert!(!rows[0].case_only);
    }

    #[test]
    fn an_occupied_target_collides() {
        let mut rows = [row(0, "/a/draft.txt", "/a/final.txt")];
        let s = check_with(&mut rows, &FsProfile::ext4(), &["/a/final.txt"]);
        assert_eq!(
            rows[0].status,
            RowStatus::Collision {
                with: vec![],
                existing: true
            }
        );
        assert_eq!(s.collisions, 1);
    }

    #[test]
    fn a_target_being_renamed_away_is_not_a_collision() {
        let mut rows = [
            row(0, "/a/b.txt", "/a/c.txt"),
            row(1, "/a/c.txt", "/a/d.txt"),
        ];
        let s = check_with(&mut rows, &FsProfile::ext4(), &["/a/b.txt", "/a/c.txt"]);
        assert_eq!(
            s.collisions, 0,
            "execution orders these; it is not a conflict"
        );
        assert_eq!(s.changed, 2);
    }

    #[test]
    fn a_swap_cycle_is_not_a_collision() {
        let mut rows = [
            row(0, "/a/a.txt", "/a/b.txt"),
            row(1, "/a/b.txt", "/a/a.txt"),
        ];
        let s = check_with(&mut rows, &FsProfile::ext4(), &["/a/a.txt", "/a/b.txt"]);
        assert_eq!(s.collisions, 0);
        assert_eq!(s.changed, 2);
    }

    #[test]
    fn windows_device_names_are_rejected_and_linux_accepts_them() {
        for name in ["CON.txt", "nul", "COM1.log", "AUX.tar.gz"] {
            let mut rows = [row(0, "/a/src.txt", &format!("/a/{name}"))];
            let s = check(&mut rows, &FsProfile::ntfs());
            assert_eq!(s.reserved, 1, "{name} must be rejected on NTFS");
            assert!(matches!(rows[0].status, RowStatus::ReservedName { .. }));
            assert!(!s.can_apply());

            let mut rows = [row(0, "/a/src.txt", &format!("/a/{name}"))];
            assert_eq!(check(&mut rows, &FsProfile::ext4()).reserved, 0);
        }
    }

    #[test]
    fn characters_windows_forbids_are_rejected() {
        for name in [
            "a:b.txt", "a?b.txt", "a*b.txt", "a<b.txt", "a|b.txt", "a\"b.txt",
        ] {
            let mut rows = [row(0, "/a/src.txt", &format!("/a/{name}"))];
            let s = check(&mut rows, &FsProfile::ntfs());
            assert_eq!(s.invalid, 1, "{name} must be invalid on NTFS");
        }
        let mut rows = [row(0, "/a/src.txt", "/a/a:b.txt")];
        assert_eq!(check(&mut rows, &FsProfile::ext4()).invalid, 0);
    }

    #[test]
    fn a_name_windows_would_shorten_to_nothing_is_invalid() {
        let mut rows = [row(0, "/a/src.txt", "/a/...")];
        let s = check(&mut rows, &FsProfile::ntfs());
        assert_eq!(s.invalid, 1);
    }

    #[test]
    fn a_trailing_dot_collides_with_the_name_windows_would_actually_write() {
        let mut rows = [
            row(0, "/a/one.txt", "/a/report."),
            row(1, "/a/two.txt", "/a/report"),
        ];
        let s = check(&mut rows, &FsProfile::ntfs());
        assert_eq!(
            s.collisions, 2,
            "NTFS drops the trailing dot, so these are the same file"
        );

        let mut rows = [
            row(0, "/a/one.txt", "/a/report."),
            row(1, "/a/two.txt", "/a/report"),
        ];
        assert_eq!(check(&mut rows, &FsProfile::ext4()).collisions, 0);
    }

    #[test]
    fn component_length_is_measured_in_the_filesystems_own_unit() {
        let emoji = "\u{1F600}".repeat(100);

        let mut rows = [row(0, "/a/src.txt", &format!("/a/{emoji}"))];
        let s = check(&mut rows, &FsProfile::ext4());
        assert_eq!(s.too_long, 1, "400 bytes exceeds ext4's 255-byte cap");

        let mut rows = [row(0, "/a/src.txt", &format!("/a/{emoji}"))];
        let s = check(&mut rows, &FsProfile::ntfs());
        assert_eq!(s.too_long, 0, "200 UTF-16 units fits NTFS's 255-unit cap");
    }

    #[test]
    fn windows_max_path_is_enforced_unless_long_paths_are_enabled() {
        let deep = format!("C:\\{}\\{}.txt", "d".repeat(140), "f".repeat(140));
        assert!(deep.len() > 260);

        let mut rows = [row(0, "C:\\a.txt", &deep)];
        let s = check_as_windows(&mut rows, false);
        assert_eq!(
            s.too_long, 1,
            "260-character MAX_PATH must be reported before Apply"
        );
        assert!(!s.can_apply());

        let mut rows = [row(0, "C:\\a.txt", &deep)];
        let s = check_as_windows(&mut rows, true);
        assert_eq!(
            s.too_long, 0,
            "with long paths opted in, the \\\\?\\ prefix reaches it"
        );
    }

    #[test]
    fn a_windows_path_is_split_on_backslashes_even_when_judged_from_linux() {
        let mut rows = [row(0, "C:\\shots\\a.txt", "C:\\shots\\CON.txt")];
        let s = check_as_windows(&mut rows, false);
        assert_eq!(s.reserved, 1, "the leaf is CON.txt, not the whole path");

        let mut rows = [row(0, "C:\\shots\\a.txt", "C:\\shots\\fine.txt")];
        assert_eq!(check_as_windows(&mut rows, false).changed, 1);
    }

    #[test]
    fn an_empty_name_is_invalid() {
        let mut rows = [row(0, "/a/src.txt", "/")];
        let s = check(&mut rows, &FsProfile::ext4());
        assert_eq!(s.invalid, 1);
    }

    #[test]
    fn a_skipped_row_keeps_its_status() {
        let mut rows = [row(0, "/a/.hidden", "/a/.hidden")];
        rows[0].status = RowStatus::Skipped {
            reason: "hidden".into(),
        };
        let s = check(&mut rows, &FsProfile::ext4());
        assert_eq!(s.skipped, 1);
        assert!(matches!(rows[0].status, RowStatus::Skipped { .. }));
    }

    #[test]
    fn revalidation_clears_a_stale_collision() {
        let profile = FsProfile::ext4();
        let mut rows = [
            row(0, "/a/x.txt", "/a/same.txt"),
            row(1, "/a/y.txt", "/a/same.txt"),
        ];
        assert_eq!(check(&mut rows, &profile).collisions, 2);

        rows[1].to = PathBuf::from("/a/other.txt");
        let s = check(&mut rows, &profile);
        assert_eq!(s.collisions, 0);
        assert_eq!(s.changed, 2);
    }

    #[test]
    fn collisions_across_different_folders_do_not_interfere() {
        let mut rows = [
            row(0, "/a/x.txt", "/a/same.txt"),
            row(1, "/b/y.txt", "/b/same.txt"),
        ];
        let s = check(&mut rows, &FsProfile::ext4());
        assert_eq!(s.collisions, 0);
        assert_eq!(s.changed, 2);
    }
}
