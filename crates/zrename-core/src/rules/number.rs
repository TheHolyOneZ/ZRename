use super::text::insert_at;
use super::{CompiledRule, RenameCtx};
use crate::error::Result;
use crate::model::{FileEntry, InsertAt, Scope, SortKey};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberParams {
    pub start: i64,
    pub step: i64,
    pub reset_per_folder: bool,
    pub sort: SortKey,
    pub descending: bool,
}

pub struct NumberRule {
    pub params: NumberParams,
    pub pad: usize,
    pub at: InsertAt,
    pub scope: Scope,
}

impl CompiledRule for NumberRule {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let text = format_number(ctx.counter, self.pad);
        let at = self.at.clone();
        ctx.map_scoped(self.scope, |s| insert_at(s, &text, &at));
        Ok(())
    }

    fn needs_ordinals(&self) -> Option<&NumberParams> {
        Some(&self.params)
    }
}

pub fn format_number(n: i64, pad: usize) -> String {
    if n < 0 {
        format!("-{:0>width$}", n.unsigned_abs(), width = pad)
    } else {
        format!("{n:0>pad$}")
    }
}

pub fn assign_ordinals(entries: &[FileEntry], p: &NumberParams) -> Vec<i64> {
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by(|&a, &b| {
        let ord = compare(&entries[a], &entries[b], p.sort);
        let ord = if ord == Ordering::Equal {
            a.cmp(&b)
        } else {
            ord
        };
        if p.descending {
            ord.reverse()
        } else {
            ord
        }
    });

    let mut out = vec![0i64; entries.len()];
    let mut per_folder: HashMap<std::path::PathBuf, i64> = HashMap::new();
    let mut global = 0i64;

    for &i in &order {
        let position = if p.reset_per_folder {
            let key = entries[i].parent();
            let slot = per_folder.entry(key).or_insert(0);
            let n = *slot;
            *slot += 1;
            n
        } else {
            let n = global;
            global += 1;
            n
        };
        out[i] = p.start + p.step * position;
    }
    out
}

fn compare(a: &FileEntry, b: &FileEntry, key: SortKey) -> Ordering {
    match key {
        SortKey::Scan => Ordering::Equal,
        SortKey::Name => a.file_name().cmp(&b.file_name()),
        SortKey::Natural => natural_cmp(&a.file_name(), &b.file_name()),
        SortKey::Size => a.size.cmp(&b.size),
        SortKey::Modified => a.mtime.cmp(&b.mtime),
        SortKey::Created => a.created.cmp(&b.created),
    }
}

pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    let (mut i, mut j) = (0usize, 0usize);

    while i < av.len() && j < bv.len() {
        if av[i].is_ascii_digit() && bv[j].is_ascii_digit() {
            let (na, ni) = take_number(&av, i);
            let (nb, nj) = take_number(&bv, j);
            match na.cmp(&nb) {
                Ordering::Equal => {
                    i = ni;
                    j = nj;
                }
                other => return other,
            }
        } else {
            let ca = av[i].to_lowercase().next().unwrap_or(av[i]);
            let cb = bv[j].to_lowercase().next().unwrap_or(bv[j]);
            match ca.cmp(&cb) {
                Ordering::Equal => {
                    i += 1;
                    j += 1;
                }
                other => return other,
            }
        }
    }
    (av.len() - i).cmp(&(bv.len() - j))
}

fn take_number(chars: &[char], mut i: usize) -> (u128, usize) {
    let mut n: u128 = 0;
    while let Some(c) = chars.get(i) {
        let Some(d) = c.to_digit(10) else { break };
        n = n.saturating_mul(10).saturating_add(d as u128);
        i += 1;
    }
    (n, i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn e(path: &str, size: u64, secs: u64) -> FileEntry {
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
            mtime: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs)),
            created: None,
            depth: 0,
        }
    }

    fn params(sort: SortKey) -> NumberParams {
        NumberParams {
            start: 1,
            step: 1,
            reset_per_folder: false,
            sort,
            descending: false,
        }
    }

    #[test]
    fn pads_to_width() {
        assert_eq!(format_number(1, 2), "01");
        assert_eq!(format_number(1, 0), "1");
        assert_eq!(format_number(100, 2), "100");
        assert_eq!(format_number(-3, 3), "-003");
    }

    #[test]
    fn natural_order_reads_digit_runs_as_numbers() {
        assert_eq!(natural_cmp("img2", "img10"), Ordering::Less);
        assert_eq!(natural_cmp("img10", "img2"), Ordering::Greater);
        assert_eq!(natural_cmp("img02", "img2"), Ordering::Equal);
        assert_eq!(natural_cmp("a", "a"), Ordering::Equal);
        assert_eq!(natural_cmp("ep9x", "ep10a"), Ordering::Less);
    }

    #[test]
    fn byte_order_and_natural_order_disagree_as_expected() {
        let mut byte = vec!["img10.jpg", "img2.jpg"];
        byte.sort();
        assert_eq!(byte, vec!["img10.jpg", "img2.jpg"]);
        let mut nat = vec!["img10.jpg", "img2.jpg"];
        nat.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(nat, vec!["img2.jpg", "img10.jpg"]);
    }

    #[test]
    fn numbers_follow_the_sort_key_but_are_returned_in_input_order() {
        let files = vec![e("/a/img10.jpg", 10, 100), e("/a/img2.jpg", 20, 200)];
        assert_eq!(
            assign_ordinals(&files, &params(SortKey::Natural)),
            vec![2, 1]
        );
        assert_eq!(assign_ordinals(&files, &params(SortKey::Name)), vec![1, 2]);
        assert_eq!(assign_ordinals(&files, &params(SortKey::Size)), vec![1, 2]);
        assert_eq!(
            assign_ordinals(&files, &params(SortKey::Modified)),
            vec![1, 2]
        );
    }

    #[test]
    fn scan_order_is_preserved_and_stable() {
        let files = vec![
            e("/a/z.jpg", 1, 1),
            e("/a/a.jpg", 2, 2),
            e("/a/m.jpg", 3, 3),
        ];
        assert_eq!(
            assign_ordinals(&files, &params(SortKey::Scan)),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn start_and_step_are_honoured() {
        let files = vec![
            e("/a/a.jpg", 1, 1),
            e("/a/b.jpg", 2, 2),
            e("/a/c.jpg", 3, 3),
        ];
        let p = NumberParams {
            start: 10,
            step: 5,
            ..params(SortKey::Name)
        };
        assert_eq!(assign_ordinals(&files, &p), vec![10, 15, 20]);
    }

    #[test]
    fn descending_reverses_the_assignment() {
        let files = vec![
            e("/a/a.jpg", 1, 1),
            e("/a/b.jpg", 2, 2),
            e("/a/c.jpg", 3, 3),
        ];
        let p = NumberParams {
            descending: true,
            ..params(SortKey::Name)
        };
        assert_eq!(assign_ordinals(&files, &p), vec![3, 2, 1]);
    }

    #[test]
    fn per_folder_reset_restarts_the_count_in_each_directory() {
        let files = vec![
            e("/a/one.jpg", 1, 1),
            e("/b/one.jpg", 1, 1),
            e("/a/two.jpg", 1, 1),
            e("/b/two.jpg", 1, 1),
        ];
        let p = NumberParams {
            reset_per_folder: true,
            ..params(SortKey::Name)
        };
        assert_eq!(assign_ordinals(&files, &p), vec![1, 1, 2, 2]);
    }

    #[test]
    fn without_reset_the_count_runs_across_folders() {
        let files = vec![e("/a/one.jpg", 1, 1), e("/b/one.jpg", 1, 1)];
        let p = params(SortKey::Name);
        let got = assign_ordinals(&files, &p);
        assert_eq!(got.len(), 2);
        assert_ne!(got[0], got[1]);
    }

    #[test]
    fn assignment_is_deterministic_across_runs() {
        let files: Vec<FileEntry> = (0..200)
            .map(|i| e(&format!("/a/f{}.jpg", i % 7), i as u64 % 3, 0))
            .collect();
        let p = params(SortKey::Size);
        let first = assign_ordinals(&files, &p);
        for _ in 0..5 {
            assert_eq!(assign_ordinals(&files, &p), first);
        }
        let mut sorted = first.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            files.len(),
            "every file must get a distinct number"
        );
    }

    #[test]
    fn an_empty_input_produces_no_numbers() {
        assert!(assign_ordinals(&[], &params(SortKey::Name)).is_empty());
    }
}
