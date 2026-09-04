use crate::dto::RowQuery;
use std::path::PathBuf;
use std::sync::Mutex;
use zrename_core::metadata::LazyMetadata;
use zrename_core::model::{FileEntry, Plan, RuleSpec};
use zrename_core::plan::PlanOptions;
use zrename_core::scan::ScanOptions;

#[derive(Default)]
pub struct Session {
    pub roots: Vec<PathBuf>,
    pub entries: Vec<FileEntry>,
    pub scan_opts: ScanOptions,
    pub rules: Vec<RuleSpec>,
    pub plan: Option<Plan>,
    pub plan_opts: Option<PlanOptions>,

    pub excluded: std::collections::HashSet<usize>,

    view: Vec<usize>,
    view_key: String,
    view_valid: bool,
}

impl Session {
    pub fn invalidate_view(&mut self) {
        self.view_valid = false;
    }

    pub fn options(&self) -> PlanOptions {
        self.plan_opts.clone().unwrap_or_default()
    }

    pub fn view(&mut self, q: &RowQuery) -> &[usize] {
        let key = q.view_key();
        if !self.view_valid || key != self.view_key {
            self.view = self.build_view(q);
            self.view_key = key;
            self.view_valid = true;
        }
        &self.view
    }

    fn build_view(&self, q: &RowQuery) -> Vec<usize> {
        let Some(plan) = &self.plan else {
            return Vec::new();
        };
        let needle = q.search.trim().to_lowercase();

        let mut idx: Vec<usize> = plan
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                if q.only_problems && !r.status.is_blocking() {
                    return false;
                }
                if q.hide_unchanged && matches!(r.status, zrename_core::model::RowStatus::Unchanged)
                {
                    return false;
                }
                if !needle.is_empty() {
                    let hit = r.from_name().to_lowercase().contains(&needle)
                        || r.to_name().to_lowercase().contains(&needle);
                    if !hit {
                        return false;
                    }
                }
                true
            })
            .map(|(i, _)| i)
            .collect();

        if q.collisions_first {
            idx.sort_by_key(|&i| {
                if plan.rows[i].status.is_blocking() {
                    0
                } else {
                    1
                }
            });
        }
        idx
    }
}

pub struct AppState {
    pub session: Mutex<Session>,
    pub meta: LazyMetadata,

    pub watch: Mutex<Option<zrename_core::watch::Watch>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(Session::default()),
            meta: LazyMetadata::new(),
            watch: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
