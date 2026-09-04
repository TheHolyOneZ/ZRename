use crate::error::{CoreError, Result};
use crate::journal::{Journal, JournalEntry};
use crate::model::{FsProfile, PlanRow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    #[default]
    Stop,
    Skip,

    Suffix,

    Overwrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Direct(usize),

    ToTemp(usize),

    FromTemp(usize),
}

impl Step {
    pub fn row(&self) -> usize {
        match *self {
            Step::Direct(i) | Step::ToTemp(i) | Step::FromTemp(i) => i,
        }
    }
}

pub fn order_steps(rows: &[PlanRow], profile: &FsProfile) -> Vec<Step> {
    let live: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.status.is_actionable())
        .map(|(i, _)| i)
        .collect();
    if live.is_empty() {
        return Vec::new();
    }

    let fold = |p: &Path| {
        let s = p.to_string_lossy();
        if profile.case_insensitive {
            s.to_lowercase()
        } else {
            s.into_owned()
        }
    };

    let mut source_of: HashMap<String, usize> = HashMap::new();
    for (node, &i) in live.iter().enumerate() {
        source_of.insert(fold(&rows[i].from), node);
    }

    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); live.len()];
    for (node, &i) in live.iter().enumerate() {
        if let Some(&occupant) = source_of.get(&fold(&rows[i].to)) {
            deps[node].push(occupant);
        }
        if rows[i].is_dir {
            let prefix = fold(&rows[i].from);
            for (other, &j) in live.iter().enumerate() {
                if other == node {
                    continue;
                }
                if is_inside(&fold(&rows[j].from), &prefix) {
                    deps[node].push(other);
                }
            }
        }
    }

    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); live.len()];
    let mut indegree: Vec<usize> = vec![0; live.len()];
    for (node, list) in deps.iter().enumerate() {
        for &d in list {
            dependents[d].push(node);
            indegree[node] += 1;
        }
    }

    let mut steps: Vec<Step> = Vec::new();
    let mut done = vec![false; live.len()];
    let mut ready: Vec<usize> = (0..live.len()).filter(|&n| indegree[n] == 0).collect();
    ready.sort_unstable();

    while let Some(node) = ready.pop() {
        if done[node] {
            continue;
        }
        done[node] = true;
        steps.push(Step::Direct(live[node]));
        for &next in &dependents[node] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                ready.push(next);
            }
        }
        ready.sort_unstable();
    }

    let stuck: Vec<usize> = (0..live.len()).filter(|&n| !done[n]).collect();
    for &n in &stuck {
        steps.push(Step::ToTemp(live[n]));
    }
    for &n in &stuck {
        steps.push(Step::FromTemp(live[n]));
    }
    steps
}

fn is_inside(path: &str, dir: &str) -> bool {
    path.len() > dir.len()
        && path.starts_with(dir)
        && matches!(path.as_bytes().get(dir.len()), Some(b'/') | Some(b'\\'))
}

#[derive(Clone, Copy, Debug)]
pub struct ExecuteOptions<'a> {
    pub conflict: ConflictPolicy,

    pub paranoid: bool,
    pub dry_run: bool,

    pub journal_dir: Option<&'a Path>,
}

impl Default for ExecuteOptions<'_> {
    fn default() -> Self {
        Self {
            conflict: ConflictPolicy::Stop,
            paranoid: false,
            dry_run: false,
            journal_dir: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExecuteReport {
    pub renamed: usize,
    pub two_phase: usize,
    pub skipped: Vec<(PathBuf, String)>,
    pub failed: Vec<(PathBuf, String)>,
    pub journal_path: Option<PathBuf>,
    pub journal_id: Option<String>,

    pub stranded: Vec<PathBuf>,
}

impl ExecuteReport {
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty() && self.stranded.is_empty()
    }
}

pub fn execute(
    rows: &[PlanRow],
    profile: &FsProfile,
    roots: &[PathBuf],
    preset: Option<String>,
    opts: &ExecuteOptions,
) -> Result<ExecuteReport> {
    let steps = order_steps(rows, profile);
    let mut report = ExecuteReport::default();
    if steps.is_empty() {
        return Ok(report);
    }

    let acting: Vec<usize> = {
        let mut v: Vec<usize> = steps.iter().map(|s| s.row()).collect();
        v.sort_unstable();
        v.dedup();
        v
    };

    let entries: Vec<JournalEntry> = acting
        .iter()
        .map(|&i| JournalEntry::capture(rows[i].from.clone(), rows[i].to.clone(), &rows[i].from))
        .collect();

    if !opts.dry_run {
        if let Some(dir) = opts.journal_dir {
            let journal = Journal::new(entries, roots.to_vec(), preset);
            report.journal_path = Some(journal.write_to(dir)?);
            report.journal_id = Some(journal.id);
        }
    }

    let _paranoid_guard = if opts.paranoid && !opts.dry_run {
        Some(hardlink_originals(rows, &acting)?)
    } else {
        None
    };

    let mut temps: HashMap<usize, PathBuf> = HashMap::new();

    for step in &steps {
        let i = step.row();
        let row = &rows[i];

        match step {
            Step::ToTemp(_) => {
                let temp = temp_path(&row.from);
                if opts.dry_run {
                    temps.insert(i, temp);
                    continue;
                }
                match rename(&row.from, &temp) {
                    Ok(()) => {
                        temps.insert(i, temp);
                    }
                    Err(e) => report.failed.push((row.from.clone(), e.to_string())),
                }
            }

            Step::FromTemp(_) => {
                let Some(temp) = temps.remove(&i) else {
                    continue;
                };
                if opts.dry_run {
                    report.renamed += 1;
                    report.two_phase += 1;
                    continue;
                }
                match settle(&temp, row, opts, &mut report) {
                    Outcome::Renamed => {
                        report.renamed += 1;
                        report.two_phase += 1;
                    }
                    Outcome::Skipped => rollback(&temp, &row.from, &mut report),
                    Outcome::Failed => rollback(&temp, &row.from, &mut report),
                    Outcome::Stop => {
                        rollback(&temp, &row.from, &mut report);
                        break;
                    }
                }
            }

            Step::Direct(_) => {
                if opts.dry_run {
                    report.renamed += 1;
                    continue;
                }
                match settle(&row.from, row, opts, &mut report) {
                    Outcome::Renamed => report.renamed += 1,
                    Outcome::Skipped | Outcome::Failed => {}
                    Outcome::Stop => break,
                }
            }
        }
    }

    for (i, temp) in temps {
        rollback(&temp, &rows[i].from, &mut report);
    }

    Ok(report)
}

enum Outcome {
    Renamed,
    Skipped,
    Failed,
    Stop,
}

fn settle(src: &Path, row: &PlanRow, opts: &ExecuteOptions, report: &mut ExecuteReport) -> Outcome {
    let target = match free_target(&row.to, opts.conflict) {
        Free::Use(p) => p,
        Free::Skip => {
            report
                .skipped
                .push((row.from.clone(), format!("{} exists", row.to.display())));
            return Outcome::Skipped;
        }
        Free::Stop => {
            report.failed.push((
                row.from.clone(),
                format!(
                    "{} exists and the conflict policy is stop",
                    row.to.display()
                ),
            ));
            return Outcome::Stop;
        }
        Free::Replace(p) => {
            let _ = if p.is_dir() {
                std::fs::remove_dir_all(&p)
            } else {
                std::fs::remove_file(&p)
            };
            p
        }
    };

    if let Some(parent) = target.parent() {
        if !parent.exists() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                report.failed.push((
                    row.from.clone(),
                    format!("creating {}: {e}", parent.display()),
                ));
                return Outcome::Failed;
            }
        }
    }

    match rename(src, &target) {
        Ok(()) => Outcome::Renamed,
        Err(e) => {
            report.failed.push((row.from.clone(), e.to_string()));
            Outcome::Failed
        }
    }
}

fn rollback(temp: &Path, original: &Path, report: &mut ExecuteReport) {
    if std::fs::rename(temp, original).is_err() {
        report.stranded.push(temp.to_path_buf());
    }
}

enum Free {
    Use(PathBuf),
    Replace(PathBuf),
    Skip,
    Stop,
}

fn free_target(to: &Path, policy: ConflictPolicy) -> Free {
    if !exists(to) {
        return Free::Use(to.to_path_buf());
    }
    match policy {
        ConflictPolicy::Stop => Free::Stop,
        ConflictPolicy::Skip => Free::Skip,
        ConflictPolicy::Overwrite => Free::Replace(to.to_path_buf()),
        ConflictPolicy::Suffix => match suffixed(to, exists) {
            Some(p) => Free::Use(p),
            None => Free::Skip,
        },
    }
}

fn exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok()
}

pub fn suffixed(to: &Path, taken: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let name = to.file_name()?.to_string_lossy().into_owned();
    let (stem, ext) = crate::model::FileEntry::split_name(&name);
    for n in 2..=9999 {
        let candidate = to.with_file_name(crate::model::FileEntry::join_name(
            &format!("{stem} ({n})"),
            ext.as_deref(),
        ));
        if !taken(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn temp_path(original: &Path) -> PathBuf {
    let dir = original
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    dir.join(format!(".zrn-{}.tmp", uuid::Uuid::new_v4()))
}

fn rename(from: &Path, to: &Path) -> Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            if from.is_dir() {
                return Err(CoreError::other(format!(
                    "{} would have to move to another filesystem, which is not supported for directories",
                    from.display()
                )));
            }
            std::fs::copy(from, to).map_err(|e| CoreError::io(to, e))?;
            std::fs::remove_file(from).map_err(|e| CoreError::io(from, e))?;
            Ok(())
        }
        Err(e) => Err(CoreError::io(from, e)),
    }
}

fn hardlink_originals(rows: &[PlanRow], acting: &[usize]) -> Result<tempfile::TempDir> {
    let dir = tempfile::Builder::new()
        .prefix("zrename-paranoid-")
        .tempdir()
        .map_err(|e| CoreError::other(format!("creating the paranoid snapshot: {e}")))?;

    for (n, &i) in acting.iter().enumerate() {
        let row = &rows[i];
        if row.is_dir {
            continue;
        }
        let link = dir.path().join(format!("{n:08}-{}", row.from_name()));
        let _ = std::fs::hard_link(&row.from, &link);
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RowStatus;

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

    fn steps_of(rows: &[PlanRow], profile: &FsProfile) -> Vec<Step> {
        order_steps(rows, profile)
    }

    #[test]
    fn independent_renames_all_go_direct() {
        let rows = [
            row(0, "/a/x.txt", "/a/1.txt"),
            row(1, "/a/y.txt", "/a/2.txt"),
        ];
        let steps = steps_of(&rows, &FsProfile::ext4());
        assert_eq!(steps.len(), 2);
        assert!(steps.iter().all(|s| matches!(s, Step::Direct(_))));
    }

    #[test]
    fn a_chain_runs_from_the_far_end_backwards() {
        let rows = [
            row(0, "/a/a.txt", "/a/b.txt"),
            row(1, "/a/b.txt", "/a/c.txt"),
        ];
        let steps = steps_of(&rows, &FsProfile::ext4());
        assert_eq!(steps, vec![Step::Direct(1), Step::Direct(0)]);
    }

    #[test]
    fn a_longer_chain_stays_correctly_ordered() {
        let rows = [
            row(0, "/a/1", "/a/2"),
            row(1, "/a/2", "/a/3"),
            row(2, "/a/3", "/a/4"),
        ];
        let steps = steps_of(&rows, &FsProfile::ext4());
        assert_eq!(
            steps,
            vec![Step::Direct(2), Step::Direct(1), Step::Direct(0)]
        );
    }

    #[test]
    fn a_two_cycle_swap_goes_through_temp_names() {
        let rows = [
            row(0, "/a/a.txt", "/a/b.txt"),
            row(1, "/a/b.txt", "/a/a.txt"),
        ];
        let steps = steps_of(&rows, &FsProfile::ext4());
        assert_eq!(
            steps,
            vec![
                Step::ToTemp(0),
                Step::ToTemp(1),
                Step::FromTemp(0),
                Step::FromTemp(1)
            ]
        );
        let first_land = steps
            .iter()
            .position(|s| matches!(s, Step::FromTemp(_)))
            .unwrap();
        let last_lift = steps
            .iter()
            .rposition(|s| matches!(s, Step::ToTemp(_)))
            .unwrap();
        assert!(
            last_lift < first_land,
            "everything must move aside before anything lands"
        );
    }

    #[test]
    fn a_three_cycle_goes_through_temp_names() {
        let rows = [
            row(0, "/a/1", "/a/2"),
            row(1, "/a/2", "/a/3"),
            row(2, "/a/3", "/a/1"),
        ];
        let steps = steps_of(&rows, &FsProfile::ext4());
        assert_eq!(
            steps
                .iter()
                .filter(|s| matches!(s, Step::ToTemp(_)))
                .count(),
            3
        );
        assert_eq!(
            steps
                .iter()
                .filter(|s| matches!(s, Step::FromTemp(_)))
                .count(),
            3
        );
        assert!(!steps.iter().any(|s| matches!(s, Step::Direct(_))));
    }

    #[test]
    fn a_case_only_change_needs_a_temp_hop_on_windows_but_not_on_linux() {
        let rows = [row(0, "/a/photo.JPG", "/a/photo.jpg")];

        let ntfs = steps_of(&rows, &FsProfile::ntfs());
        assert_eq!(
            ntfs,
            vec![Step::ToTemp(0), Step::FromTemp(0)],
            "\u{a7}2's silent-failure trap"
        );

        let ext4 = steps_of(&rows, &FsProfile::ext4());
        assert_eq!(
            ext4,
            vec![Step::Direct(0)],
            "a case-sensitive filesystem needs no dance"
        );
    }

    #[test]
    fn a_cycle_mixed_with_independent_renames_only_costs_the_cycle() {
        let rows = [
            row(0, "/a/a.txt", "/a/b.txt"),
            row(1, "/a/b.txt", "/a/a.txt"),
            row(2, "/a/z.txt", "/a/q.txt"),
        ];
        let steps = steps_of(&rows, &FsProfile::ext4());
        assert_eq!(steps[0], Step::Direct(2));
        assert_eq!(
            steps
                .iter()
                .filter(|s| matches!(s, Step::ToTemp(_)))
                .count(),
            2
        );
    }

    #[test]
    fn a_directory_is_renamed_after_its_contents() {
        let mut rows = [
            row(0, "/a/old", "/a/new"),
            row(1, "/a/old/file.txt", "/a/old/renamed.txt"),
        ];
        rows[0].is_dir = true;
        let steps = steps_of(&rows, &FsProfile::ext4());
        assert_eq!(steps, vec![Step::Direct(1), Step::Direct(0)]);
    }

    #[test]
    fn non_actionable_rows_are_not_executed() {
        let mut rows = [
            row(0, "/a/a.txt", "/a/b.txt"),
            row(1, "/a/c.txt", "/a/c.txt"),
            row(2, "/a/d.txt", "/a/e.txt"),
        ];
        rows[1].status = RowStatus::Unchanged;
        rows[2].status = RowStatus::Collision {
            with: vec![],
            existing: true,
        };
        let steps = steps_of(&rows, &FsProfile::ext4());
        assert_eq!(steps, vec![Step::Direct(0)]);
    }

    #[test]
    fn an_empty_plan_produces_no_steps() {
        assert!(order_steps(&[], &FsProfile::ext4()).is_empty());
    }

    #[test]
    fn every_actionable_row_is_covered_exactly_once() {
        let rows = [
            row(0, "/a/a", "/a/b"),
            row(1, "/a/b", "/a/a"),
            row(2, "/a/c", "/a/d"),
            row(3, "/a/d", "/a/e"),
        ];
        let steps = steps_of(&rows, &FsProfile::ext4());
        for i in 0..4 {
            let lands = steps
                .iter()
                .filter(|s| s.row() == i && matches!(s, Step::Direct(_) | Step::FromTemp(_)))
                .count();
            assert_eq!(lands, 1, "row {i} must land exactly once");
        }
    }

    #[test]
    fn suffix_numbering_starts_at_two_and_skips_what_is_taken() {
        let to = PathBuf::from("/a/scan-03.pdf");
        let taken = |p: &Path| {
            matches!(
                p.file_name().unwrap().to_str().unwrap(),
                "scan-03 (2).pdf" | "scan-03 (3).pdf"
            )
        };
        assert_eq!(
            suffixed(&to, taken).unwrap(),
            PathBuf::from("/a/scan-03 (4).pdf")
        );
        assert_eq!(
            suffixed(&to, |_| false).unwrap(),
            PathBuf::from("/a/scan-03 (2).pdf")
        );
    }

    #[test]
    fn suffix_numbering_handles_a_name_with_no_extension() {
        let to = PathBuf::from("/a/README");
        assert_eq!(
            suffixed(&to, |_| false).unwrap(),
            PathBuf::from("/a/README (2)")
        );
    }

    #[test]
    fn containment_check_needs_a_separator_boundary() {
        assert!(is_inside("/a/old/file.txt", "/a/old"));
        assert!(!is_inside("/a/older/file.txt", "/a/old"));
        assert!(!is_inside("/a/old", "/a/old"));
        assert!(is_inside("C:\\a\\old\\f.txt", "C:\\a\\old"));
    }
}
