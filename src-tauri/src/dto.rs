use serde::{Deserialize, Serialize};
use zrename_core::export;
use zrename_core::model::{DiffSpan, Plan, PlanRow, RowStatus};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub total: usize,
    pub files: usize,
    pub folders: usize,
    pub roots: Vec<String>,
    pub fs_name: String,
    pub case_insensitive: bool,
    pub max_path: Option<usize>,

    pub needs_sanitising: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryDto {
    pub total: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub collisions: usize,
    pub invalid: usize,
    pub skipped: usize,
    pub too_long: usize,
    pub reserved: usize,
    pub blocking: usize,
    pub can_apply: bool,
    pub summary_line: String,
    pub apply_label: String,
    pub fs_name: String,
}

impl SummaryDto {
    pub fn of(plan: &Plan) -> Self {
        let s = &plan.summary;
        Self {
            total: s.total,
            changed: s.changed,
            unchanged: s.unchanged,
            collisions: s.collisions,
            invalid: s.invalid,
            skipped: s.skipped,
            too_long: s.too_long,
            reserved: s.reserved,
            blocking: s.blocking(),
            can_apply: s.can_apply(),
            summary_line: export::summary_line(plan),
            apply_label: export::apply_label(plan),
            fs_name: plan.profile.name.clone(),
        }
    }

    pub fn empty() -> Self {
        Self {
            total: 0,
            changed: 0,
            unchanged: 0,
            collisions: 0,
            invalid: 0,
            skipped: 0,
            too_long: 0,
            reserved: 0,
            blocking: 0,
            can_apply: false,
            summary_line: "Nothing loaded".into(),
            apply_label: "Apply".into(),
            fs_name: String::new(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowDto {
    pub index: usize,
    pub from_name: String,
    pub to_name: String,
    pub from_path: String,
    pub to_path: String,

    pub status: String,
    pub status_label: String,
    pub blocking: bool,
    pub actionable: bool,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub case_only: bool,
    pub moved: bool,
    pub excluded: bool,
    pub diff: Vec<DiffSpan>,
}

impl RowDto {
    pub fn of(row: &PlanRow, excluded: bool) -> Self {
        let from_name = row.from_name();
        let to_name = row.to_name();
        let diff = zrename_core::diff::diff(&from_name, &to_name);
        Self {
            index: row.index,
            from_path: row.from.to_string_lossy().into_owned(),
            to_path: row.to.to_string_lossy().into_owned(),
            status: status_key(&row.status).into(),
            status_label: export::status_label(&row.status),
            blocking: row.status.is_blocking(),
            actionable: row.status.is_actionable(),
            is_dir: row.is_dir,
            is_symlink: row.is_symlink,
            case_only: row.case_only,
            moved: row.from.parent() != row.to.parent(),
            excluded,
            from_name,
            to_name,
            diff,
        }
    }
}

pub fn status_key(s: &RowStatus) -> &'static str {
    match s {
        RowStatus::Ok => "ok",
        RowStatus::Unchanged => "unchanged",
        RowStatus::Collision { .. } => "collision",
        RowStatus::Invalid { .. } => "invalid",
        RowStatus::TooLong { .. } => "too_long",
        RowStatus::ReservedName { .. } => "reserved_name",
        RowStatus::Skipped { .. } => "skipped",
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RowQuery {
    pub offset: usize,
    pub limit: usize,
    #[serde(default)]
    pub hide_unchanged: bool,
    #[serde(default)]
    pub only_problems: bool,
    #[serde(default)]
    pub search: String,

    #[serde(default)]
    pub collisions_first: bool,
}

impl RowQuery {
    pub fn view_key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.hide_unchanged, self.only_problems, self.collisions_first, self.search
        )
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowPage {
    pub rows: Vec<RowDto>,

    pub total: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegexTest {
    pub valid: bool,
    pub error: Option<String>,
    pub matched: bool,
    pub groups: Vec<String>,
    pub preview: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub renamed: usize,
    pub two_phase: usize,
    pub skipped: Vec<[String; 2]>,
    pub failed: Vec<[String; 2]>,
    pub stranded: Vec<String>,
    pub journal_id: Option<String>,
    pub clean: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoResult {
    pub reverted: usize,
    pub total: usize,
    pub skipped: Vec<SkipDto>,
    pub failed: Vec<[String; 2]>,
    pub clean: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkipDto {
    pub name: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub created: String,
    pub count: usize,
    pub preset: Option<String>,
    pub roots: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupeGroupDto {
    pub hash: String,
    pub size: u64,
    pub names: Vec<String>,
    pub paths: Vec<String>,

    pub indices: Vec<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub ffprobe: bool,
    pub preset_dir: String,
    pub journal_dir: String,
    pub version: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupArgs {
    pub paths: Vec<String>,
    pub preset: Option<String>,
}
