use crate::error::{CoreError, Result};
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{
    new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Watch {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    pub roots: Vec<PathBuf>,
}

pub const DEFAULT_SETTLE: Duration = Duration::from_secs(2);

pub fn watch<F>(
    roots: &[PathBuf],
    recursive: bool,
    settle: Duration,
    on_arrival: F,
) -> Result<Watch>
where
    F: Fn(Vec<PathBuf>) + Send + 'static,
{
    let mut debouncer = new_debouncer(settle, None, move |res: DebounceEventResult| {
        let Ok(events) = res else { return };
        let paths = arrivals(&events);
        if !paths.is_empty() {
            on_arrival(paths);
        }
    })
    .map_err(|e| CoreError::other(format!("could not start watching: {e}")))?;

    let mode = if recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    for root in roots {
        debouncer
            .watch(root, mode)
            .map_err(|e| CoreError::other(format!("could not watch {}: {e}", root.display())))?;
    }

    Ok(Watch {
        _debouncer: debouncer,
        roots: roots.to_vec(),
    })
}

pub fn arrivals(events: &[DebouncedEvent]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for event in events {
        if !is_arrival_kind(&event.kind) {
            continue;
        }
        for path in &event.paths {
            if !is_candidate(path) {
                continue;
            }
            if !out.contains(path) {
                out.push(path.clone());
            }
        }
    }
    out
}

fn is_arrival_kind(kind: &notify::EventKind) -> bool {
    use notify::event::{CreateKind, EventKind, ModifyKind, RenameMode};
    matches!(
        kind,
        EventKind::Create(CreateKind::File | CreateKind::Any)
            | EventKind::Modify(ModifyKind::Name(RenameMode::To))
    )
}

pub fn is_candidate(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    if name.starts_with(".zrn-") {
        return false;
    }
    if name.starts_with('.') {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    for suffix in [".part", ".crdownload", ".download", ".tmp", ".partial", "~"] {
        if lower.ends_with(suffix) {
            return false;
        }
    }

    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind, RenameMode};
    use notify::Event;
    use std::time::Instant;

    fn event(kind: EventKind, paths: &[&Path]) -> DebouncedEvent {
        DebouncedEvent {
            event: Event {
                kind,
                paths: paths.iter().map(|p| p.to_path_buf()).collect(),
                attrs: Default::default(),
            },
            time: Instant::now(),
        }
    }

    #[test]
    fn a_created_file_counts_as_an_arrival() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("photo.jpg");
        std::fs::write(&p, b"x").unwrap();

        let got = arrivals(&[event(EventKind::Create(CreateKind::File), &[&p])]);
        assert_eq!(got, vec![p]);
    }

    #[test]
    fn a_file_renamed_into_the_folder_counts() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("moved.jpg");
        std::fs::write(&p, b"x").unwrap();

        let got = arrivals(&[event(
            EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            &[&p],
        )]);
        assert_eq!(got, vec![p]);
    }

    #[test]
    fn deletions_are_not_arrivals() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("gone.jpg");
        std::fs::write(&p, b"x").unwrap();

        assert!(arrivals(&[event(EventKind::Remove(RemoveKind::File), &[&p])]).is_empty());
    }

    #[test]
    fn our_own_two_phase_temporaries_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let temp = dir.path().join(".zrn-1234.tmp");
        std::fs::write(&temp, b"x").unwrap();

        assert!(
            arrivals(&[event(EventKind::Create(CreateKind::File), &[&temp])]).is_empty(),
            "a watch that acted on its own temporaries would chase its own tail"
        );
    }

    #[test]
    fn half_written_downloads_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "big.iso.part",
            "movie.mp4.crdownload",
            "notes.txt~",
            "draft.TMP",
        ] {
            let p = dir.path().join(name);
            std::fs::write(&p, b"x").unwrap();
            assert!(
                !is_candidate(&p),
                "{name} should be left alone until it settles"
            );
        }
    }

    #[test]
    fn directories_and_missing_paths_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        assert!(!is_candidate(&sub));
        assert!(!is_candidate(&dir.path().join("never-existed.jpg")));
    }

    #[test]
    fn the_same_path_is_reported_once_per_batch() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.jpg");
        std::fs::write(&p, b"x").unwrap();

        let got = arrivals(&[
            event(EventKind::Create(CreateKind::File), &[&p]),
            event(EventKind::Create(CreateKind::File), &[&p]),
        ]);
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn a_real_watch_sees_a_file_appear() {
        let dir = tempfile::tempdir().unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<PathBuf>::new()));
        let sink = seen.clone();

        let _w = watch(
            &[dir.path().to_path_buf()],
            false,
            Duration::from_millis(120),
            move |paths| sink.lock().unwrap().extend(paths),
        )
        .unwrap();

        std::fs::write(dir.path().join("arrived.jpg"), b"x").unwrap();

        for _ in 0..60 {
            if !seen.lock().unwrap().is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let got = seen.lock().unwrap();
        assert!(
            got.iter().any(|p| p.file_name().unwrap() == "arrived.jpg"),
            "the watch should have reported the new file, saw {got:?}"
        );
    }
}
