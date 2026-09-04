pub mod diff;
pub mod dupes;
pub mod error;
pub mod execute;
pub mod export;
pub mod fsinfo;
pub mod journal;
pub mod metadata;
pub mod model;
pub mod plan;
pub mod presets;
pub mod rules;
pub mod scan;
pub mod tokens;
pub mod validate;
pub mod watch;

pub use error::{CoreError, Result};
pub use execute::{execute, ConflictPolicy, ExecuteOptions, ExecuteReport};
pub use journal::{Journal, UndoOptions, UndoReport};
pub use metadata::LazyMetadata;
pub use model::{
    CaseStyle, DiffOp, DiffSpan, ExtMode, FileEntry, FsProfile, InsertAt, LengthLimit, LengthUnit,
    MissingToken, Plan, PlanRow, PlanSummary, RemoveWhat, RowStatus, RuleKind, RuleSpec, Scope,
    SortKey,
};
pub use plan::{build_plan, PlanOptions};
pub use presets::Preset;
pub use scan::{scan, ScanOptions};
pub use tokens::{MetadataProvider, NullProvider};
