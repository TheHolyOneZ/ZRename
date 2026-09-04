use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub stem: String,
    pub ext: Option<String>,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    #[serde(skip)]
    pub mtime: Option<SystemTime>,
    #[serde(skip)]
    pub created: Option<SystemTime>,
    pub depth: usize,
}

impl FileEntry {
    pub fn split_name(file_name: &str) -> (String, Option<String>) {
        let body = file_name
            .strip_prefix('.')
            .map(|r| (1usize, r))
            .unwrap_or((0, file_name));
        let (offset, rest) = body;
        match rest.rfind('.') {
            Some(i) if i + 1 < rest.len() => {
                let cut = offset + i;
                (
                    file_name[..cut].to_string(),
                    Some(file_name[cut + 1..].to_string()),
                )
            }
            _ => (file_name.to_string(), None),
        }
    }

    pub fn join_name(stem: &str, ext: Option<&str>) -> String {
        match ext {
            Some(e) if !e.is_empty() => format!("{stem}.{e}"),
            _ => stem.to_string(),
        }
    }

    pub fn file_name(&self) -> String {
        Self::join_name(&self.stem, self.ext.as_deref())
    }

    pub fn parent(&self) -> PathBuf {
        self.path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scope {
    #[serde(default = "yes")]
    pub stem: bool,
    #[serde(default)]
    pub ext: bool,
}

impl Default for Scope {
    fn default() -> Self {
        Self {
            stem: true,
            ext: false,
        }
    }
}

impl Scope {
    pub const STEM: Self = Self {
        stem: true,
        ext: false,
    };
    pub const EXT: Self = Self {
        stem: false,
        ext: true,
    };
    pub const BOTH: Self = Self {
        stem: true,
        ext: true,
    };
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaseStyle {
    Lower,
    Upper,
    Title,
    Sentence,
    Camel,
    Pascal,
    Snake,
    Kebab,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, Default)]
#[serde(tag = "at", rename_all = "snake_case")]
pub enum InsertAt {
    Index {
        index: i64,
    },
    Before {
        marker: String,
        all: bool,
    },
    After {
        marker: String,
        all: bool,
    },
    Prefix,
    #[default]
    Suffix,
}

impl<'de> Deserialize<'de> for InsertAt {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            at: Option<String>,
            #[serde(default)]
            index: Option<i64>,
            #[serde(default)]
            marker: Option<String>,
            #[serde(default)]
            all: bool,
        }

        let raw = Raw::deserialize(d)?;
        let marker = raw.marker.unwrap_or_default();
        Ok(match raw.at.as_deref() {
            Some("index") => InsertAt::Index {
                index: raw.index.unwrap_or(0),
            },
            Some("before") => InsertAt::Before {
                marker,
                all: raw.all,
            },
            Some("after") => InsertAt::After {
                marker,
                all: raw.all,
            },
            Some("prefix") => InsertAt::Prefix,
            Some("suffix") | None => InsertAt::Suffix,
            Some(other) => {
                return Err(serde::de::Error::custom(format!(
                    "unknown insert position `{other}`"
                )))
            }
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "what", rename_all = "snake_case")]
pub enum RemoveWhat {
    Range {
        from: i64,
        to: i64,
    },
    Chars {
        chars: String,
    },
    Word {
        word: String,
        #[serde(default = "yes")]
        all: bool,
    },
    Digits,
    Duplicates {
        text: String,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    #[default]
    Name,

    Natural,
    Size,
    Modified,
    Created,

    Scan,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ExtMode {
    Set { ext: String },
    Lower,
    Upper,
    Remove,

    Fill { ext: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuleKind {
    Replace {
        find: String,
        #[serde(default)]
        with: String,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default = "yes")]
        all: bool,
    },
    Case {
        style: CaseStyle,
    },
    Insert {
        text: String,
        #[serde(flatten)]
        at: InsertAt,
    },
    Remove {
        #[serde(flatten)]
        what: RemoveWhat,
    },
    Trim {
        #[serde(default = "yes")]
        whitespace: bool,
        #[serde(default)]
        chars: String,
        #[serde(default)]
        collapse_spaces: bool,
    },
    Number {
        #[serde(default = "one")]
        start: i64,
        #[serde(default = "one")]
        step: i64,
        #[serde(default = "one_usize")]
        pad: usize,
        #[serde(default)]
        reset_per_folder: bool,
        #[serde(default)]
        sort: SortKey,
        #[serde(default)]
        descending: bool,
        #[serde(flatten)]
        at: InsertAt,
    },
    Extension {
        #[serde(flatten)]
        mode: ExtMode,
    },
    Sanitise {
        #[serde(default = "yes")]
        illegal: bool,
        #[serde(default = "yes")]
        collapse_spaces: bool,
        #[serde(default)]
        transliterate: bool,
        #[serde(default = "underscore")]
        replacement: String,
        #[serde(default = "yes")]
        trim_dots_spaces: bool,
    },

    Template {
        template: String,
    },

    CsvMap {
        path: PathBuf,
        #[serde(default)]
        match_full_name: bool,
    },

    MoveInto {
        template: String,
    },
}

fn one() -> i64 {
    1
}
fn one_usize() -> usize {
    1
}
fn underscore() -> String {
    "_".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RuleSpec {
    #[serde(default = "new_id")]
    pub id: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub scope: Scope,
    #[serde(flatten)]
    pub kind: RuleKind,
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

impl RuleSpec {
    pub fn new(kind: RuleKind) -> Self {
        Self {
            id: new_id(),
            enabled: true,
            scope: Scope::default(),
            kind,
        }
    }

    pub fn with_scope(mut self, scope: Scope) -> Self {
        self.scope = scope;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffSpan {
    pub op: DiffOp,
    pub text: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffOp {
    Equal,
    Insert,
    Delete,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RowStatus {
    Ok,
    Unchanged,

    Collision {
        with: Vec<usize>,
        existing: bool,
    },
    Invalid {
        reason: String,
    },
    TooLong {
        limit: usize,
        actual: usize,
        unit: LengthUnit,
    },
    ReservedName {
        name: String,
    },
    Skipped {
        reason: String,
    },
}

impl RowStatus {
    pub fn is_actionable(&self) -> bool {
        matches!(self, RowStatus::Ok)
    }

    pub fn is_blocking(&self) -> bool {
        matches!(
            self,
            RowStatus::Collision { .. }
                | RowStatus::Invalid { .. }
                | RowStatus::TooLong { .. }
                | RowStatus::ReservedName { .. }
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanRow {
    pub index: usize,
    pub from: PathBuf,
    pub to: PathBuf,
    pub status: RowStatus,
    pub is_dir: bool,

    pub is_symlink: bool,

    pub case_only: bool,
}

impl PlanRow {
    pub fn from_name(&self) -> String {
        self.from
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn to_name(&self) -> String {
        self.to
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct PlanSummary {
    pub total: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub collisions: usize,
    pub invalid: usize,
    pub skipped: usize,
    pub too_long: usize,
    pub reserved: usize,
}

impl PlanSummary {
    pub fn can_apply(&self) -> bool {
        self.changed > 0 && self.blocking() == 0
    }

    pub fn blocking(&self) -> usize {
        self.collisions + self.invalid + self.too_long + self.reserved
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub rows: Vec<PlanRow>,
    pub summary: PlanSummary,
    pub profile: FsProfile,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MissingToken {
    #[default]
    Placeholder,

    Skip,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LengthUnit {
    Bytes,
    Utf16Units,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LengthLimit {
    pub unit: LengthUnit,
    pub max: usize,
}

impl LengthLimit {
    pub const fn bytes(max: usize) -> Self {
        Self {
            unit: LengthUnit::Bytes,
            max,
        }
    }

    pub const fn utf16(max: usize) -> Self {
        Self {
            unit: LengthUnit::Utf16Units,
            max,
        }
    }

    pub fn measure(&self, s: &str) -> usize {
        match self.unit {
            LengthUnit::Bytes => s.len(),
            LengthUnit::Utf16Units => s.encode_utf16().count(),
        }
    }

    pub fn exceeded_by(&self, s: &str) -> Option<usize> {
        let n = self.measure(s);
        (n > self.max).then_some(n)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FsProfile {
    pub name: String,
    pub case_insensitive: bool,
    pub illegal_chars: Vec<char>,

    pub reserved_stems: Vec<String>,

    pub strips_trailing_dot_space: bool,
    pub max_component: LengthLimit,
    pub max_path: Option<usize>,

    pub supports_long_path_prefix: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_ordinary_name() {
        assert_eq!(
            FileEntry::split_name("IMG_4821.JPG"),
            ("IMG_4821".into(), Some("JPG".into()))
        );
    }

    #[test]
    fn last_dot_wins() {
        assert_eq!(
            FileEntry::split_name("archive.tar.gz"),
            ("archive.tar".into(), Some("gz".into()))
        );
    }

    #[test]
    fn leading_dot_is_not_an_extension() {
        assert_eq!(FileEntry::split_name(".hidden"), (".hidden".into(), None));
        assert_eq!(
            FileEntry::split_name(".hidden.txt"),
            (".hidden".into(), Some("txt".into()))
        );
        assert_eq!(
            FileEntry::split_name(".config.d"),
            (".config".into(), Some("d".into()))
        );
    }

    #[test]
    fn no_extension_and_trailing_dot() {
        assert_eq!(FileEntry::split_name("Makefile"), ("Makefile".into(), None));
        assert_eq!(FileEntry::split_name("weird."), ("weird.".into(), None));
    }

    #[test]
    fn join_is_the_inverse_of_split() {
        for name in [
            "IMG_4821.JPG",
            "archive.tar.gz",
            ".hidden",
            ".hidden.txt",
            "Makefile",
            "weird.",
        ] {
            let (stem, ext) = FileEntry::split_name(name);
            assert_eq!(
                FileEntry::join_name(&stem, ext.as_deref()),
                name,
                "roundtrip failed for {name}"
            );
        }
    }

    #[test]
    fn length_limit_counts_in_its_own_unit() {
        let emoji = "\u{1F600}".repeat(100);
        assert_eq!(LengthLimit::bytes(255).measure(&emoji), 400);
        assert_eq!(LengthLimit::utf16(255).measure(&emoji), 200);
        assert!(LengthLimit::bytes(255).exceeded_by(&emoji).is_some());
        assert!(LengthLimit::utf16(255).exceeded_by(&emoji).is_none());
    }

    #[test]
    fn summary_gates_apply_on_blocking_rows() {
        let mut s = PlanSummary {
            total: 10,
            changed: 9,
            ..Default::default()
        };
        assert!(s.can_apply());
        s.collisions = 1;
        assert!(!s.can_apply());
    }
}
