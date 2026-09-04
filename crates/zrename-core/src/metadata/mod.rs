pub mod audio;
pub mod exif;
pub mod hash;
pub mod pdf;
pub mod video;

use crate::model::FileEntry;
use crate::tokens::{shorthand_date_format, MetadataProvider};
use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub type Values = HashMap<String, String>;

#[derive(Default)]
pub struct LazyMetadata {
    cache: Mutex<HashMap<(PathBuf, String), Arc<Values>>>,
    ffprobe: Option<PathBuf>,
}

impl LazyMetadata {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            ffprobe: video::find_ffprobe(),
        }
    }

    pub fn has_ffprobe(&self) -> bool {
        self.ffprobe.is_some()
    }

    pub fn clear(&self) {
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
    }

    fn values(&self, path: &Path, ns: &str) -> Arc<Values> {
        let key = (path.to_path_buf(), ns.to_string());
        if let Ok(cache) = self.cache.lock() {
            if let Some(hit) = cache.get(&key) {
                return hit.clone();
            }
        }

        let extracted = Arc::new(self.extract(path, ns));
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key, extracted.clone());
        }
        extracted
    }

    fn extract(&self, path: &Path, ns: &str) -> Values {
        match ns {
            "exif" => exif::read(path),
            "id3" | "audio" => audio::read(path),
            "pdf" => pdf::read(path),
            "video" => match &self.ffprobe {
                Some(bin) => video::read(bin, path),
                None => Values::new(),
            },
            _ => Values::new(),
        }
    }
}

impl MetadataProvider for LazyMetadata {
    fn resolve(&self, entry: &FileEntry, ns: &str, key: &str, fmt: Option<&str>) -> Option<String> {
        if ns == "hash" {
            return hash::compute(&entry.path, key);
        }

        let values = self.values(&entry.path, ns);
        if values.is_empty() {
            return None;
        }

        if let Some(shorthand) = shorthand_date_format(key) {
            let raw = lookup(&values, default_date_key(ns))?;
            return Some(format_value(raw, Some(&shorthand)));
        }

        let raw = lookup(&values, key)?;
        Some(format_value(raw, fmt))
    }
}

fn default_date_key(ns: &str) -> &'static str {
    match ns {
        "exif" => "DateTimeOriginal",
        "id3" | "audio" => "year",
        "pdf" => "creationdate",
        _ => "date",
    }
}

pub fn lookup<'a>(values: &'a Values, key: &str) -> Option<&'a str> {
    let wanted = key.to_ascii_lowercase();
    values.get(&wanted).map(|s| s.as_str())
}

pub fn put(values: &mut Values, key: &str, value: impl Into<String>) {
    let value = value.into();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    values.insert(key.to_ascii_lowercase(), trimmed.to_string());
}

pub fn format_value(raw: &str, fmt: Option<&str>) -> String {
    let Some(f) = fmt else { return raw.to_string() };
    match parse_datetime(raw) {
        Some(dt) => dt.format(f).to_string(),
        None => raw.to_string(),
    }
}

pub fn parse_datetime(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    const DATETIME: &[&str] = &[
        "%Y:%m:%d %H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y%m%d%H%M%S",
        "%Y:%m:%d %H:%M",
    ];
    for f in DATETIME {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, f) {
            return Some(dt);
        }
    }
    const DATE: &[&str] = &["%Y-%m-%d", "%Y:%m:%d", "%Y%m%d"];
    for f in DATE {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, f) {
            return d.and_hms_opt(0, 0, 0);
        }
    }

    if s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(year) = s.parse::<i32>() {
            return chrono::NaiveDate::from_ymd_opt(year, 1, 1)?.and_hms_opt(0, 0, 0);
        }
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.naive_local());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exif_style_timestamps_parse() {
        let dt = parse_datetime("2026:08:14 10:23:45").unwrap();
        assert_eq!(dt.format("%Y-%m-%d").to_string(), "2026-08-14");
        assert_eq!(dt.format("%H%M%S").to_string(), "102345");
    }

    #[test]
    fn other_common_shapes_parse_too() {
        for s in [
            "2026-08-14 10:23:45",
            "2026-08-14T10:23:45",
            "20260814102345",
            "2026-08-14",
        ] {
            let dt = parse_datetime(s).unwrap_or_else(|| panic!("{s} should parse"));
            assert_eq!(dt.format("%Y-%m-%d").to_string(), "2026-08-14");
        }
        assert_eq!(
            parse_datetime("2026").unwrap().format("%Y").to_string(),
            "2026"
        );
    }

    #[test]
    fn a_value_that_is_not_a_date_survives_formatting_unchanged() {
        assert_eq!(
            format_value("Canon EOS R6", Some("%Y-%m-%d")),
            "Canon EOS R6"
        );
        assert_eq!(format_value("Canon EOS R6", None), "Canon EOS R6");
        assert!(parse_datetime("Canon EOS R6").is_none());
    }

    #[test]
    fn formatting_only_applies_when_a_format_is_given() {
        assert_eq!(
            format_value("2026:08:14 10:23:45", None),
            "2026:08:14 10:23:45"
        );
        assert_eq!(
            format_value("2026:08:14 10:23:45", Some("%Y/%m")),
            "2026/08"
        );
    }

    #[test]
    fn keys_are_matched_without_regard_to_case() {
        let mut v = Values::new();
        put(&mut v, "DateTimeOriginal", "2026:08:14 10:23:45");
        put(&mut v, "Model", "Canon EOS R6");
        assert!(lookup(&v, "model").is_some());
        assert!(lookup(&v, "MODEL").is_some());
        assert!(lookup(&v, "datetimeoriginal").is_some());
        assert!(lookup(&v, "nothing").is_none());
    }

    #[test]
    fn blank_values_are_not_stored_so_they_resolve_to_the_placeholder() {
        let mut v = Values::new();
        put(&mut v, "Artist", "   ");
        put(&mut v, "Album", "");
        put(&mut v, "Title", "  Real  ");
        assert!(lookup(&v, "artist").is_none());
        assert!(lookup(&v, "album").is_none());
        assert_eq!(lookup(&v, "title").unwrap(), "Real");
    }

    #[test]
    fn a_file_with_no_metadata_resolves_to_nothing_rather_than_failing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("plain.txt");
        std::fs::write(&p, b"not a photo").unwrap();

        let entry = FileEntry {
            path: p,
            stem: "plain".into(),
            ext: Some("txt".into()),
            is_dir: false,
            is_symlink: false,
            size: 11,
            mtime: None,
            created: None,
            depth: 0,
        };

        let m = LazyMetadata::new();
        assert!(m.resolve(&entry, "exif", "Model", None).is_none());
        assert!(m.resolve(&entry, "id3", "artist", None).is_none());
        assert!(m.resolve(&entry, "pdf", "title", None).is_none());
    }

    #[test]
    fn hashes_resolve_and_are_stable() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.bin");
        std::fs::write(&p, b"hello").unwrap();

        let entry = FileEntry {
            path: p,
            stem: "a".into(),
            ext: Some("bin".into()),
            is_dir: false,
            is_symlink: false,
            size: 5,
            mtime: None,
            created: None,
            depth: 0,
        };

        let m = LazyMetadata::new();
        let crc = m.resolve(&entry, "hash", "crc32", None).unwrap();
        assert_eq!(crc, "3610a686", "crc32 of \"hello\"");
        assert_eq!(m.resolve(&entry, "hash", "crc32", None).unwrap(), crc);
        assert_eq!(m.resolve(&entry, "hash", "blake3", None).unwrap().len(), 16);
        assert!(m.resolve(&entry, "hash", "nonsense", None).is_none());
    }

    #[test]
    fn the_default_date_key_differs_by_namespace() {
        assert_eq!(default_date_key("exif"), "DateTimeOriginal");
        assert_eq!(default_date_key("id3"), "year");
    }
}
