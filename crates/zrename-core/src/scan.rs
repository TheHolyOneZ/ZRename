use crate::error::Result;
use crate::model::FileEntry;
use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanOptions {
    #[serde(default)]
    pub recursive: bool,

    #[serde(default)]
    pub max_depth: Option<usize>,
    #[serde(default = "yes")]
    pub include_files: bool,
    #[serde(default)]
    pub include_dirs: bool,
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(default)]
    pub follow_symlinks: bool,
    #[serde(default)]
    pub include_globs: Vec<String>,
    #[serde(default)]
    pub exclude_globs: Vec<String>,

    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub name_regex: Option<String>,
    #[serde(default)]
    pub min_size: Option<u64>,
    #[serde(default)]
    pub max_size: Option<u64>,
    #[serde(default)]
    pub modified_after: Option<SystemTime>,
    #[serde(default)]
    pub modified_before: Option<SystemTime>,
}

fn yes() -> bool {
    true
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            recursive: false,
            max_depth: None,
            include_files: true,
            include_dirs: false,
            include_hidden: false,
            follow_symlinks: false,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            extensions: Vec::new(),
            name_regex: None,
            min_size: None,
            max_size: None,
            modified_after: None,
            modified_before: None,
        }
    }
}

pub struct Filter {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
    extensions: HashSet<String>,
    name_re: Option<Regex>,
    include_files: bool,
    include_dirs: bool,
    include_hidden: bool,
    min_size: Option<u64>,
    max_size: Option<u64>,
    modified_after: Option<SystemTime>,
    modified_before: Option<SystemTime>,
}

impl Filter {
    pub fn compile(o: &ScanOptions) -> Result<Self> {
        Ok(Self {
            include: build_globs(&o.include_globs)?,
            exclude: build_globs(&o.exclude_globs)?,
            extensions: o
                .extensions
                .iter()
                .map(|e| e.trim_start_matches('.').to_lowercase())
                .collect(),
            name_re: o
                .name_regex
                .as_deref()
                .filter(|s| !s.is_empty())
                .map(Regex::new)
                .transpose()?,
            include_files: o.include_files,
            include_dirs: o.include_dirs,
            include_hidden: o.include_hidden,
            min_size: o.min_size,
            max_size: o.max_size,
            modified_after: o.modified_after,
            modified_before: o.modified_before,
        })
    }

    pub fn matches(&self, e: &FileEntry) -> bool {
        if e.is_dir && !self.include_dirs {
            return false;
        }
        if !e.is_dir && !self.include_files {
            return false;
        }

        let name = e.file_name();
        if !self.include_hidden && name.starts_with('.') {
            return false;
        }

        if !self.extensions.is_empty() {
            let ext = e.ext.as_deref().unwrap_or("").to_lowercase();
            if !self.extensions.contains(&ext) {
                return false;
            }
        }

        if let Some(re) = &self.name_re {
            if !re.is_match(&name) {
                return false;
            }
        }

        if let Some(set) = &self.include {
            if !set.is_match(&name) && !set.is_match(&e.path) {
                return false;
            }
        }
        if let Some(set) = &self.exclude {
            if set.is_match(&name) || set.is_match(&e.path) {
                return false;
            }
        }

        if !e.is_dir {
            if self.min_size.is_some_and(|m| e.size < m) {
                return false;
            }
            if self.max_size.is_some_and(|m| e.size > m) {
                return false;
            }
        }

        if let Some(after) = self.modified_after {
            if e.mtime.is_none_or(|t| t < after) {
                return false;
            }
        }
        if let Some(before) = self.modified_before {
            if e.mtime.is_none_or(|t| t > before) {
                return false;
            }
        }

        true
    }
}

fn build_globs(patterns: &[String]) -> Result<Option<GlobSet>> {
    let live: Vec<&String> = patterns.iter().filter(|p| !p.trim().is_empty()).collect();
    if live.is_empty() {
        return Ok(None);
    }
    let mut b = GlobSetBuilder::new();
    for p in live {
        b.add(Glob::new(p.trim())?);
    }
    Ok(Some(b.build()?))
}

pub fn entry_from(
    path: &Path,
    meta: &std::fs::Metadata,
    depth: usize,
    is_symlink: bool,
) -> FileEntry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (stem, ext) = if meta.is_dir() {
        (name.clone(), None)
    } else {
        FileEntry::split_name(&name)
    };
    FileEntry {
        path: path.to_path_buf(),
        stem,
        ext,
        is_dir: meta.is_dir(),
        is_symlink,
        size: meta.len(),
        mtime: meta.modified().ok(),
        created: meta.created().ok(),
        depth,
    }
}

pub fn scan(roots: &[PathBuf], opts: &ScanOptions) -> Result<Vec<FileEntry>> {
    let filter = Filter::compile(opts)?;
    let mut out: Vec<FileEntry> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    for root in roots {
        let root_meta = match std::fs::symlink_metadata(root) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if !root_meta.is_dir() {
            let target = std::fs::metadata(root).unwrap_or_else(|_| root_meta.clone());
            let e = entry_from(root, &target, 0, root_meta.file_type().is_symlink());
            if seen.insert(e.path.clone()) {
                out.push(e);
            }
            continue;
        }

        let depth_limit = if opts.recursive {
            opts.max_depth.map(|d| d + 1)
        } else {
            Some(1)
        };
        let mut walker = walkdir::WalkDir::new(root)
            .min_depth(1)
            .follow_links(opts.follow_symlinks);
        if let Some(d) = depth_limit {
            walker = walker.max_depth(d);
        }

        for item in walker.sort_by_file_name() {
            let Ok(item) = item else { continue };
            let Ok(meta) = item.metadata() else { continue };
            let e = entry_from(item.path(), &meta, item.depth(), item.path_is_symlink());
            if !filter.matches(&e) {
                continue;
            }
            if seen.insert(e.path.clone()) {
                out.push(e);
            }
        }
    }

    out.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.path.cmp(&b.path)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn e(name: &str, is_dir: bool, size: u64) -> FileEntry {
        let (stem, ext) = if is_dir {
            (name.to_string(), None)
        } else {
            FileEntry::split_name(name)
        };
        FileEntry {
            path: PathBuf::from(format!("/root/{name}")),
            stem,
            ext,
            is_dir,
            is_symlink: false,
            size,
            mtime: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1000)),
            created: None,
            depth: 1,
        }
    }

    fn filter(o: ScanOptions) -> Filter {
        Filter::compile(&o).unwrap()
    }

    #[test]
    fn files_pass_and_directories_do_not_by_default() {
        let f = filter(ScanOptions::default());
        assert!(f.matches(&e("a.txt", false, 10)));
        assert!(!f.matches(&e("sub", true, 0)));
    }

    #[test]
    fn directories_can_be_included_for_renaming() {
        let f = filter(ScanOptions {
            include_dirs: true,
            include_files: false,
            ..Default::default()
        });
        assert!(f.matches(&e("sub", true, 0)));
        assert!(!f.matches(&e("a.txt", false, 10)));
    }

    #[test]
    fn hidden_files_are_skipped_unless_asked_for() {
        let f = filter(ScanOptions::default());
        assert!(!f.matches(&e(".hidden", false, 1)));
        let f = filter(ScanOptions {
            include_hidden: true,
            ..Default::default()
        });
        assert!(f.matches(&e(".hidden", false, 1)));
    }

    #[test]
    fn extension_filtering_ignores_case_and_a_leading_dot() {
        let o = ScanOptions {
            extensions: vec!["jpg".into(), ".PNG".into(), "pdf".into()],
            ..Default::default()
        };
        let f = filter(o);
        assert!(f.matches(&e("a.jpg", false, 1)));
        assert!(f.matches(&e("a.JPG", false, 1)));
        assert!(f.matches(&e("a.png", false, 1)));
        assert!(f.matches(&e("a.pdf", false, 1)));
        assert!(!f.matches(&e("a.gif", false, 1)));
        assert!(!f.matches(&e("Makefile", false, 1)));
    }

    #[test]
    fn include_and_exclude_globs_combine_with_exclude_winning() {
        let o = ScanOptions {
            include_globs: vec!["IMG_*".into()],
            exclude_globs: vec!["*_edit.*".into()],
            ..Default::default()
        };
        let f = filter(o);
        assert!(f.matches(&e("IMG_1.jpg", false, 1)));
        assert!(!f.matches(&e("DSC_1.jpg", false, 1)));
        assert!(!f.matches(&e("IMG_1_edit.jpg", false, 1)));
    }

    #[test]
    fn a_name_regex_narrows_the_selection() {
        let o = ScanOptions {
            name_regex: Some(r"^S\d{2}E\d{2}".into()),
            ..Default::default()
        };
        let f = filter(o);
        assert!(f.matches(&e("S01E02 - pilot.mkv", false, 1)));
        assert!(!f.matches(&e("bloopers.mkv", false, 1)));
    }

    #[test]
    fn size_bounds_apply_to_files_only() {
        let o = ScanOptions {
            min_size: Some(100),
            max_size: Some(1000),
            include_dirs: true,
            ..Default::default()
        };
        let f = filter(o);
        assert!(!f.matches(&e("small.txt", false, 99)));
        assert!(f.matches(&e("ok.txt", false, 500)));
        assert!(!f.matches(&e("big.txt", false, 1001)));
        assert!(
            f.matches(&e("sub", true, 0)),
            "a directory's size is meaningless"
        );
    }

    #[test]
    fn modification_time_bounds_are_inclusive_of_the_range() {
        let t = |s| SystemTime::UNIX_EPOCH + Duration::from_secs(s);
        let f = filter(ScanOptions {
            modified_after: Some(t(500)),
            ..Default::default()
        });
        assert!(f.matches(&e("a.txt", false, 1)));
        let f = filter(ScanOptions {
            modified_after: Some(t(2000)),
            ..Default::default()
        });
        assert!(!f.matches(&e("a.txt", false, 1)));
        let f = filter(ScanOptions {
            modified_before: Some(t(2000)),
            ..Default::default()
        });
        assert!(f.matches(&e("a.txt", false, 1)));
        let f = filter(ScanOptions {
            modified_before: Some(t(500)),
            ..Default::default()
        });
        assert!(!f.matches(&e("a.txt", false, 1)));
    }

    #[test]
    fn an_invalid_glob_or_regex_is_reported() {
        assert!(Filter::compile(&ScanOptions {
            include_globs: vec!["[".into()],
            ..Default::default()
        })
        .is_err());
        assert!(Filter::compile(&ScanOptions {
            name_regex: Some("(unclosed".into()),
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn blank_patterns_are_ignored_rather_than_matching_nothing() {
        let o = ScanOptions {
            include_globs: vec!["".into(), "  ".into()],
            name_regex: Some("".into()),
            ..Default::default()
        };
        let f = filter(o);
        assert!(f.matches(&e("anything.txt", false, 1)));
    }

    #[test]
    fn scanning_a_real_tree_respects_depth() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub/deeper")).unwrap();
        std::fs::write(root.join("top.txt"), b"1").unwrap();
        std::fs::write(root.join("sub/mid.txt"), b"2").unwrap();
        std::fs::write(root.join("sub/deeper/low.txt"), b"3").unwrap();

        let names = |o: &ScanOptions| {
            let mut v: Vec<String> = scan(&[root.to_path_buf()], o)
                .unwrap()
                .iter()
                .map(|e| e.file_name())
                .collect();
            v.sort();
            v
        };

        assert_eq!(names(&ScanOptions::default()), vec!["top.txt"]);
        assert_eq!(
            names(&ScanOptions {
                recursive: true,
                ..Default::default()
            }),
            vec!["low.txt", "mid.txt", "top.txt"]
        );
        assert_eq!(
            names(&ScanOptions {
                recursive: true,
                max_depth: Some(1),
                ..Default::default()
            }),
            vec!["mid.txt", "top.txt"]
        );
    }

    #[test]
    fn a_dropped_file_is_taken_even_though_it_is_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = dir.path().join(".dotfile");
        std::fs::write(&hidden, b"x").unwrap();

        let got = scan(std::slice::from_ref(&hidden), &ScanOptions::default()).unwrap();
        assert_eq!(
            got.len(),
            1,
            "an explicitly chosen file is not second-guessed"
        );
        assert_eq!(got[0].path, hidden);
    }

    #[test]
    fn the_same_file_reached_twice_appears_once() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), b"x").unwrap();
        let roots = vec![dir.path().to_path_buf(), dir.path().to_path_buf()];
        assert_eq!(scan(&roots, &ScanOptions::default()).unwrap().len(), 1);
    }

    #[test]
    fn scan_order_is_shallow_first_then_alphabetical() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        for n in ["b.txt", "a.txt"] {
            std::fs::write(root.join(n), b"x").unwrap();
        }
        std::fs::write(root.join("sub/c.txt"), b"x").unwrap();

        let got = scan(
            &[root.to_path_buf()],
            &ScanOptions {
                recursive: true,
                ..Default::default()
            },
        )
        .unwrap();
        let names: Vec<String> = got.iter().map(|e| e.file_name()).collect();
        assert_eq!(names, vec!["a.txt", "b.txt", "c.txt"]);
    }
}
