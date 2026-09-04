use crate::model::FileEntry;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub size: u64,

    pub members: Vec<usize>,
}

pub fn find_duplicates(entries: &[FileEntry]) -> Vec<DuplicateGroup> {
    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if e.is_dir || e.is_symlink {
            continue;
        }
        by_size.entry(e.size).or_default().push(i);
    }

    let candidates: Vec<usize> = by_size
        .values()
        .filter(|v| v.len() > 1)
        .flat_map(|v| v.iter().copied())
        .collect();

    let hashed: Vec<(usize, String)> = candidates
        .par_iter()
        .filter_map(|&i| hash_file(&entries[i].path).map(|h| (i, h)))
        .collect();

    let mut by_hash: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, h) in hashed {
        by_hash.entry(h).or_default().push(i);
    }

    let mut groups: Vec<DuplicateGroup> = by_hash
        .into_iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(hash, mut members)| {
            members.sort_unstable();
            DuplicateGroup {
                hash,
                size: entries[members[0]].size,
                members,
            }
        })
        .collect();

    groups.sort_by_key(|g| g.members[0]);
    groups
}

fn hash_file(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 128 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hasher.finalize().to_hex().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{scan, ScanOptions};

    fn tree(files: &[(&str, &[u8])]) -> (tempfile::TempDir, Vec<FileEntry>) {
        let dir = tempfile::tempdir().unwrap();
        for (name, body) in files {
            std::fs::write(dir.path().join(name), body).unwrap();
        }
        let entries = scan(&[dir.path().to_path_buf()], &ScanOptions::default()).unwrap();
        (dir, entries)
    }

    #[test]
    fn identical_content_is_grouped_whatever_the_names() {
        let (_d, entries) = tree(&[
            ("a.txt", b"same content"),
            ("b.txt", b"same content"),
            ("c.txt", b"different"),
        ]);
        let groups = find_duplicates(&entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members.len(), 2);

        let names: Vec<String> = groups[0]
            .members
            .iter()
            .map(|&i| entries[i].file_name())
            .collect();
        assert!(names.contains(&"a.txt".to_string()));
        assert!(names.contains(&"b.txt".to_string()));
    }

    #[test]
    fn files_of_the_same_length_but_different_content_are_not_duplicates() {
        let (_d, entries) = tree(&[("a.txt", b"aaaa"), ("b.txt", b"bbbb")]);
        assert!(find_duplicates(&entries).is_empty());
    }

    #[test]
    fn three_copies_form_one_group_not_three_pairs() {
        let (_d, entries) = tree(&[
            ("a.txt", b"x"),
            ("b.txt", b"x"),
            ("c.txt", b"x"),
            ("d.txt", b"y"),
        ]);
        let groups = find_duplicates(&entries);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members.len(), 3);
    }

    #[test]
    fn empty_files_are_duplicates_of_each_other() {
        let (_d, entries) = tree(&[("a.txt", b""), ("b.txt", b"")]);
        assert_eq!(find_duplicates(&entries).len(), 1);
    }

    #[test]
    fn a_single_file_and_an_empty_set_produce_no_groups() {
        let (_d, entries) = tree(&[("only.txt", b"x")]);
        assert!(find_duplicates(&entries).is_empty());
        assert!(find_duplicates(&[]).is_empty());
    }

    #[test]
    fn results_are_stable_across_runs_despite_parallel_hashing() {
        let bodies: Vec<Vec<u8>> = (0..40).map(|i| vec![(i % 5) as u8; 1000]).collect();
        let dir = tempfile::tempdir().unwrap();
        for (i, b) in bodies.iter().enumerate() {
            std::fs::write(dir.path().join(format!("f{i:02}.bin")), b).unwrap();
        }
        let entries = scan(&[dir.path().to_path_buf()], &ScanOptions::default()).unwrap();

        let first = find_duplicates(&entries);
        assert_eq!(first.len(), 5, "five distinct contents, eight copies each");
        for _ in 0..3 {
            let again = find_duplicates(&entries);
            let a: Vec<&Vec<usize>> = first.iter().map(|g| &g.members).collect();
            let b: Vec<&Vec<usize>> = again.iter().map(|g| &g.members).collect();
            assert_eq!(a, b);
        }
    }
}
