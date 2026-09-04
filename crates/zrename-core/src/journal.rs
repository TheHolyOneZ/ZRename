use crate::error::{CoreError, Result};
use chrono::{DateTime, Local, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct JournalEntry {
    pub from: PathBuf,
    pub to: PathBuf,
    pub size: u64,

    pub mtime: Option<i64>,
    pub inode: Option<u64>,
    #[serde(default)]
    pub is_dir: bool,
}

impl JournalEntry {
    pub fn capture(from: PathBuf, to: PathBuf, path: &Path) -> Self {
        let meta = std::fs::symlink_metadata(path).ok();
        Self {
            from,
            to,
            size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            mtime: meta.as_ref().and_then(|m| m.modified().ok()).map(to_secs),
            inode: meta.as_ref().and_then(inode_of),
            is_dir: meta.as_ref().is_some_and(|m| m.is_dir()),
        }
    }
}

pub fn to_secs(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    }
}

#[cfg(unix)]
fn inode_of(m: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(m.ino())
}

#[cfg(windows)]
fn inode_of(_m: &std::fs::Metadata) -> Option<u64> {
    None
}

#[cfg(not(any(unix, windows)))]
fn inode_of(_m: &std::fs::Metadata) -> Option<u64> {
    None
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Journal {
    pub version: u32,

    pub id: String,
    pub created: String,
    pub roots: Vec<PathBuf>,
    pub preset: Option<String>,
    pub entries: Vec<JournalEntry>,
}

impl Journal {
    pub fn new(entries: Vec<JournalEntry>, roots: Vec<PathBuf>, preset: Option<String>) -> Self {
        let now: DateTime<Local> = Local::now();
        Self {
            version: VERSION,
            id: now.format("%Y-%m-%dT%H-%M-%S%.3f").to_string(),
            created: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            roots,
            preset,
            entries,
        }
    }

    pub fn write_to(&self, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir).map_err(|e| CoreError::io(dir, e))?;
        let path = dir.join(format!("{}.json", self.id));
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| CoreError::Journal(format!("serialising: {e}")))?;

        let mut f = std::fs::File::create(&path).map_err(|e| CoreError::io(&path, e))?;
        f.write_all(&body).map_err(|e| CoreError::io(&path, e))?;
        f.sync_all().map_err(|e| CoreError::io(&path, e))?;
        Ok(path)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let body = std::fs::read(path).map_err(|e| CoreError::io(path, e))?;
        serde_json::from_slice(&body)
            .map_err(|e| CoreError::Journal(format!("{}: {e}", path.display())))
    }

    pub fn summary(&self, path: PathBuf) -> JournalSummary {
        JournalSummary {
            id: self.id.clone(),
            created: self.created.clone(),
            count: self.entries.len(),
            preset: self.preset.clone(),
            roots: self.roots.clone(),
            path,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JournalSummary {
    pub id: String,
    pub created: String,
    pub count: usize,
    pub preset: Option<String>,
    pub roots: Vec<PathBuf>,
    pub path: PathBuf,
}

pub fn default_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| CoreError::Journal("no local data directory on this system".into()))?;
    Ok(base.join("zrename").join("journal"))
}

pub fn list(dir: &Path) -> Result<Vec<JournalSummary>> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<JournalSummary> = Vec::new();
    for item in rd.flatten() {
        let path = item.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(j) = Journal::load(&path) {
            out.push(j.summary(path));
        }
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(out)
}

pub fn prune(dir: &Path, keep: usize) -> Result<usize> {
    let all = list(dir)?;
    let mut removed = 0;
    for s in all.into_iter().skip(keep) {
        if std::fs::remove_file(&s.path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftKind {
    Missing,

    Modified,

    OldNameTaken,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UndoSkip {
    pub entry: JournalEntry,
    pub kind: DriftKind,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UndoReport {
    pub reverted: usize,
    pub skipped: Vec<UndoSkip>,
    pub failed: Vec<(PathBuf, String)>,
}

impl UndoReport {
    pub fn is_clean(&self) -> bool {
        self.skipped.is_empty() && self.failed.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct UndoOptions {
    pub force: bool,
    pub dry_run: bool,
}

pub fn undo(journal: &Journal, opts: &UndoOptions) -> UndoReport {
    let mut report = UndoReport::default();

    for entry in journal.entries.iter().rev() {
        match check_entry(entry, opts.force) {
            Err(skip) => report.skipped.push(*skip),
            Ok(()) => {
                if opts.dry_run {
                    report.reverted += 1;
                    continue;
                }
                if let Some(parent) = entry.from.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::rename(&entry.to, &entry.from) {
                    Ok(()) => report.reverted += 1,
                    Err(e) => report.failed.push((entry.to.clone(), e.to_string())),
                }
            }
        }
    }
    report
}

fn check_entry(entry: &JournalEntry, force: bool) -> std::result::Result<(), Box<UndoSkip>> {
    let skip = |kind, detail: String| {
        Box::new(UndoSkip {
            entry: entry.clone(),
            kind,
            detail,
        })
    };

    let Ok(meta) = std::fs::symlink_metadata(&entry.to) else {
        return Err(skip(
            DriftKind::Missing,
            format!("{} is gone", entry.to.display()),
        ));
    };

    if !force {
        if meta.len() != entry.size {
            return Err(skip(
                DriftKind::Modified,
                format!("size is now {}, was {}", meta.len(), entry.size),
            ));
        }
        if let (Some(recorded), Ok(actual)) = (entry.mtime, meta.modified()) {
            let actual = to_secs(actual);
            if actual != recorded {
                return Err(skip(
                    DriftKind::Modified,
                    format!("modified at {actual}, journal recorded {recorded}"),
                ));
            }
        }
    }

    if entry.from != entry.to && std::fs::symlink_metadata(&entry.from).is_ok() {
        return Err(skip(
            DriftKind::OldNameTaken,
            format!("{} exists again", entry.from.display()),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path, body: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn do_renames(pairs: &[(PathBuf, PathBuf)]) -> Journal {
        let entries: Vec<JournalEntry> = pairs
            .iter()
            .map(|(from, to)| {
                let e = JournalEntry::capture(from.clone(), to.clone(), from);
                std::fs::rename(from, to).unwrap();
                e
            })
            .collect();
        Journal::new(entries, vec![], None)
    }

    #[test]
    fn a_batch_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::new(
            vec![JournalEntry {
                from: PathBuf::from("/a/one.txt"),
                to: PathBuf::from("/a/two.txt"),
                size: 3,
                mtime: Some(1700),
                inode: Some(42),
                is_dir: false,
            }],
            vec![PathBuf::from("/a")],
            Some("Photos".into()),
        );
        let path = j.write_to(dir.path()).unwrap();
        assert!(path.exists());

        let back = Journal::load(&path).unwrap();
        assert_eq!(back.entries, j.entries);
        assert_eq!(back.preset.as_deref(), Some("Photos"));
        assert_eq!(back.version, VERSION);
    }

    #[test]
    fn undo_puts_every_file_back() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for n in 1..=3 {
            touch(&root.join(format!("IMG_{n}.JPG")), b"photo");
        }
        let pairs: Vec<(PathBuf, PathBuf)> = (1..=3)
            .map(|n| {
                (
                    root.join(format!("IMG_{n}.JPG")),
                    root.join(format!("2026-08-14_0{n}.jpg")),
                )
            })
            .collect();

        let journal = do_renames(&pairs);
        assert!(root.join("2026-08-14_01.jpg").exists());
        assert!(!root.join("IMG_1.JPG").exists());

        let report = undo(&journal, &UndoOptions::default());
        assert_eq!(report.reverted, 3);
        assert!(report.is_clean());
        for n in 1..=3 {
            assert!(root.join(format!("IMG_{n}.JPG")).exists());
            assert!(!root.join(format!("2026-08-14_0{n}.jpg")).exists());
        }
    }

    #[test]
    fn undo_survives_a_restart_because_it_only_needs_the_file_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("a.txt"), b"x");

        let journal = do_renames(&[(root.join("a.txt"), root.join("b.txt"))]);
        let path = journal.write_to(store.path()).unwrap();

        let reloaded = Journal::load(&path).unwrap();
        let report = undo(&reloaded, &UndoOptions::default());
        assert_eq!(report.reverted, 1);
        assert!(root.join("a.txt").exists());
    }

    #[test]
    fn undo_refuses_to_overwrite_a_file_that_changed_underneath() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("a.txt"), b"original");
        let journal = do_renames(&[(root.join("a.txt"), root.join("b.txt"))]);

        std::fs::write(root.join("b.txt"), b"edited since the rename").unwrap();

        let report = undo(&journal, &UndoOptions::default());
        assert_eq!(report.reverted, 0);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].kind, DriftKind::Modified);
        assert!(!report.is_clean());
        assert_eq!(
            std::fs::read(root.join("b.txt")).unwrap(),
            b"edited since the rename",
            "the edited file must be left exactly as it was"
        );
    }

    #[test]
    fn force_reverts_a_modified_file_when_explicitly_asked() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("a.txt"), b"original");
        let journal = do_renames(&[(root.join("a.txt"), root.join("b.txt"))]);
        std::fs::write(root.join("b.txt"), b"edited").unwrap();

        let report = undo(
            &journal,
            &UndoOptions {
                force: true,
                dry_run: false,
            },
        );
        assert_eq!(report.reverted, 1);
        assert!(root.join("a.txt").exists());
    }

    #[test]
    fn undo_reports_a_file_that_vanished() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("a.txt"), b"x");
        let journal = do_renames(&[(root.join("a.txt"), root.join("b.txt"))]);
        std::fs::remove_file(root.join("b.txt")).unwrap();

        let report = undo(&journal, &UndoOptions::default());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].kind, DriftKind::Missing);
    }

    #[test]
    fn undo_will_not_clobber_a_new_file_that_took_the_old_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("a.txt"), b"x");
        let journal = do_renames(&[(root.join("a.txt"), root.join("b.txt"))]);
        touch(&root.join("a.txt"), b"a different file entirely");

        let report = undo(
            &journal,
            &UndoOptions {
                force: true,
                dry_run: false,
            },
        );
        assert_eq!(report.reverted, 0);
        assert_eq!(report.skipped[0].kind, DriftKind::OldNameTaken);
        assert_eq!(
            std::fs::read(root.join("a.txt")).unwrap(),
            b"a different file entirely",
            "even force must not destroy an unrecorded file"
        );
    }

    #[test]
    fn a_dry_run_undo_reports_without_moving_anything() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("a.txt"), b"x");
        let journal = do_renames(&[(root.join("a.txt"), root.join("b.txt"))]);

        let report = undo(
            &journal,
            &UndoOptions {
                force: false,
                dry_run: true,
            },
        );
        assert_eq!(report.reverted, 1);
        assert!(root.join("b.txt").exists(), "nothing should have moved");
        assert!(!root.join("a.txt").exists());
    }

    #[test]
    fn undo_replays_a_chain_in_reverse() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("a.txt"), b"A");
        touch(&root.join("b.txt"), b"B");

        let journal = do_renames(&[
            (root.join("b.txt"), root.join("c.txt")),
            (root.join("a.txt"), root.join("b.txt")),
        ]);

        let report = undo(&journal, &UndoOptions::default());
        assert_eq!(report.reverted, 2);
        assert!(report.is_clean());
        assert_eq!(std::fs::read(root.join("a.txt")).unwrap(), b"A");
        assert_eq!(std::fs::read(root.join("b.txt")).unwrap(), b"B");
        assert!(!root.join("c.txt").exists());
    }

    #[test]
    fn history_lists_newest_first_and_prunes_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            let mut j = Journal::new(vec![], vec![], Some(format!("batch{i}")));
            j.id = format!("2026-09-0{}T10-00-00", i + 1);
            j.write_to(dir.path()).unwrap();
        }

        let all = list(dir.path()).unwrap();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0].preset.as_deref(), Some("batch4"), "newest first");
        assert_eq!(all[4].preset.as_deref(), Some("batch0"));

        assert_eq!(prune(dir.path(), 2).unwrap(), 3);
        let kept = list(dir.path()).unwrap();
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].preset.as_deref(), Some("batch4"));
    }

    #[test]
    fn listing_a_directory_that_does_not_exist_yet_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(&dir.path().join("nope")).unwrap().is_empty());
    }

    #[test]
    fn a_corrupt_journal_file_is_ignored_rather_than_breaking_history() {
        let dir = tempfile::tempdir().unwrap();
        Journal::new(vec![], vec![], Some("good".into()))
            .write_to(dir.path())
            .unwrap();
        std::fs::write(dir.path().join("broken.json"), b"{not json").unwrap();

        let all = list(dir.path()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].preset.as_deref(), Some("good"));
    }

    #[test]
    fn the_default_location_matches_the_spec() {
        let dir = default_dir().unwrap();
        assert!(dir.ends_with("zrename/journal"), "got {}", dir.display());
    }

    #[test]
    fn entry_capture_records_identity_from_the_live_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        touch(&p, b"12345");
        let e = JournalEntry::capture(p.clone(), dir.path().join("b.txt"), &p);
        assert_eq!(e.size, 5);
        assert!(e.mtime.is_some());
        assert!(!e.is_dir);
    }
}
