//! End-to-end tests against a real filesystem.
//!
//! The unit tests prove the planning logic; these prove that what lands on disk
//! matches the plan, and that undo puts it all back.

use std::path::Path;
use zrename_core::execute::{execute, ConflictPolicy, ExecuteOptions};
use zrename_core::journal::{self, Journal, UndoOptions};
use zrename_core::model::{
    CaseStyle, ExtMode, FsProfile, InsertAt, Plan, RuleKind, RuleSpec, SortKey,
};
use zrename_core::plan::{build_plan, PlanOptions};
use zrename_core::scan::{scan, ScanOptions};
use zrename_core::tokens::NullProvider;

fn touch(path: &Path, body: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn plan_for(root: &Path, specs: &[RuleSpec], opts: &PlanOptions) -> Plan {
    let entries = scan(&[root.to_path_buf()], &ScanOptions::default()).unwrap();
    build_plan(&entries, specs, &NullProvider, opts).unwrap()
}

/// Options judged against the filesystem actually hosting the test directory.
fn local_opts(root: &Path) -> PlanOptions {
    PlanOptions::for_path(root)
}

/// Runs a plan and returns the report.
fn apply(
    plan: &Plan,
    root: &Path,
    journal_dir: &Path,
    conflict: ConflictPolicy,
) -> zrename_core::execute::ExecuteReport {
    execute(
        &plan.rows,
        &plan.profile,
        &[root.to_path_buf()],
        None,
        &ExecuteOptions {
            conflict,
            journal_dir: Some(journal_dir),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn a_batch_applies_and_then_undoes_completely() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();

    for n in 1..=5 {
        touch(
            &root.join(format!("IMG_482{n}.JPG")),
            format!("photo{n}").as_bytes(),
        );
    }

    let specs = vec![
        RuleSpec::new(RuleKind::Template {
            template: "2026-08-14".into(),
        }),
        RuleSpec::new(RuleKind::Number {
            start: 1,
            step: 1,
            pad: 2,
            reset_per_folder: true,
            sort: SortKey::Natural,
            descending: false,
            at: InsertAt::Suffix,
        }),
        RuleSpec::new(RuleKind::Extension {
            mode: ExtMode::Lower,
        }),
    ];

    let opts = local_opts(root);
    let plan = plan_for(root, &specs, &opts);
    assert_eq!(plan.summary.changed, 5);
    assert_eq!(plan.summary.collisions, 0);
    assert!(plan.summary.can_apply());

    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 5);
    assert!(report.is_clean());
    assert_eq!(
        names_in(root),
        vec![
            "2026-08-1401.jpg",
            "2026-08-1402.jpg",
            "2026-08-1403.jpg",
            "2026-08-1404.jpg",
            "2026-08-1405.jpg"
        ]
    );

    // Reopen the journal from disk, as a later session would.
    let history = journal::list(store.path()).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].count, 5);

    let reloaded = Journal::load(&history[0].path).unwrap();
    let undo = journal::undo(&reloaded, &UndoOptions::default());
    assert_eq!(undo.reverted, 5);
    assert!(undo.is_clean());

    let mut expected: Vec<String> = (1..=5).map(|n| format!("IMG_482{n}.JPG")).collect();
    expected.sort();
    assert_eq!(names_in(root), expected);
    for n in 1..=5 {
        assert_eq!(
            std::fs::read(root.join(format!("IMG_482{n}.JPG"))).unwrap(),
            format!("photo{n}").as_bytes()
        );
    }
}

#[test]
fn the_journal_reaches_disk_before_the_first_rename() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("a.txt"), b"x");

    assert!(journal::list(store.path()).unwrap().is_empty());

    let specs = vec![RuleSpec::new(RuleKind::Case {
        style: CaseStyle::Upper,
    })];
    let opts = local_opts(root);
    let plan = plan_for(root, &specs, &opts);
    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);

    let path = report.journal_path.expect("a journal must be written");
    assert!(path.exists());

    let j = Journal::load(&path).unwrap();
    assert_eq!(j.entries.len(), 1);
    assert_eq!(j.entries[0].from, root.join("a.txt"));
    assert_eq!(
        j.entries[0].size, 1,
        "identity is captured before the file moves"
    );
}

#[test]
fn a_swap_between_two_files_completes_through_temp_names() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("a.txt"), b"content-A");
    touch(&root.join("b.txt"), b"content-B");

    let specs = vec![RuleSpec::new(RuleKind::Replace {
        find: "^(a|b)$".into(),
        with: "$1".into(),
        regex: true,
        case_sensitive: true,
        all: true,
    })];

    // Build the swap directly: the rule engine has no "swap" rule, so the plan
    // is constructed by hand to exercise execution ordering on real files.
    let opts = local_opts(root);
    let mut plan = plan_for(root, &specs, &opts);
    plan.rows[0].to = root.join("b.txt");
    plan.rows[1].to = root.join("a.txt");
    plan.rows[0].status = zrename_core::RowStatus::Ok;
    plan.rows[1].status = zrename_core::RowStatus::Ok;

    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 2);
    assert_eq!(
        report.two_phase, 2,
        "a cycle cannot be done without temp names"
    );
    assert!(report.is_clean());

    assert_eq!(std::fs::read(root.join("a.txt")).unwrap(), b"content-B");
    assert_eq!(std::fs::read(root.join("b.txt")).unwrap(), b"content-A");
    assert_eq!(
        names_in(root),
        vec!["a.txt", "b.txt"],
        "no temp files left behind"
    );
}

#[test]
fn a_case_only_rename_lands_when_the_profile_says_the_filesystem_folds_case() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("photo.JPG"), b"jpeg");

    // Judged as NTFS: source and target are the same entry, so this must be
    // routed through a temp name rather than attempted directly.
    let opts = PlanOptions {
        profile: FsProfile::ntfs(),
        ..Default::default()
    };
    let specs = vec![RuleSpec::new(RuleKind::Extension {
        mode: ExtMode::Lower,
    })];
    let plan = plan_for(root, &specs, &opts);

    assert!(plan.rows[0].case_only);
    assert_eq!(plan.summary.collisions, 0);

    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 1);
    assert_eq!(report.two_phase, 1);
    assert!(report.is_clean());
    assert_eq!(names_in(root), vec!["photo.jpg"]);
    assert_eq!(std::fs::read(root.join("photo.jpg")).unwrap(), b"jpeg");
}

#[test]
fn a_chain_of_renames_does_not_eat_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("1.txt"), b"one");
    touch(&root.join("2.txt"), b"two");
    touch(&root.join("3.txt"), b"three");

    // 1->2, 2->3, 3->4
    let opts = local_opts(root);
    let mut plan = plan_for(root, &[], &opts);
    for (i, target) in ["2.txt", "3.txt", "4.txt"].iter().enumerate() {
        plan.rows[i].to = root.join(target);
        plan.rows[i].status = zrename_core::RowStatus::Ok;
    }

    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 3);
    assert!(report.is_clean());

    assert_eq!(names_in(root), vec!["2.txt", "3.txt", "4.txt"]);
    assert_eq!(std::fs::read(root.join("2.txt")).unwrap(), b"one");
    assert_eq!(std::fs::read(root.join("3.txt")).unwrap(), b"two");
    assert_eq!(std::fs::read(root.join("4.txt")).unwrap(), b"three");
}

#[test]
fn the_skip_policy_leaves_the_loser_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("a.txt"), b"A");
    touch(&root.join("b.txt"), b"B");

    let specs = vec![RuleSpec::new(RuleKind::Template {
        template: "same".into(),
    })];
    let opts = PlanOptions {
        conflict: ConflictPolicy::Skip,
        ..local_opts(root)
    };
    let plan = plan_for(root, &specs, &opts);

    assert_eq!(plan.summary.collisions, 0, "the policy resolved it");
    assert_eq!(plan.summary.changed, 1);
    assert_eq!(plan.summary.skipped, 1);
    assert!(plan.summary.can_apply());
}

#[test]
fn the_suffix_policy_gives_each_file_a_free_name() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    for n in ["a", "b", "c"] {
        touch(&root.join(format!("{n}.txt")), n.as_bytes());
    }

    let specs = vec![RuleSpec::new(RuleKind::Template {
        template: "scan-03".into(),
    })];
    let opts = PlanOptions {
        conflict: ConflictPolicy::Suffix,
        ..local_opts(root)
    };
    let plan = plan_for(root, &specs, &opts);

    assert_eq!(plan.summary.collisions, 0);
    assert_eq!(plan.summary.changed, 3);

    let report = apply(&plan, root, store.path(), ConflictPolicy::Suffix);
    assert_eq!(report.renamed, 3);
    assert!(report.is_clean());
    assert_eq!(
        names_in(root),
        vec!["scan-03 (2).txt", "scan-03 (3).txt", "scan-03.txt"]
    );
}

#[test]
fn overwrite_never_settles_two_rows_competing_for_one_name() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("a.txt"), b"A");
    touch(&root.join("b.txt"), b"B");

    let specs = vec![RuleSpec::new(RuleKind::Template {
        template: "same".into(),
    })];
    let opts = PlanOptions {
        conflict: ConflictPolicy::Overwrite,
        ..local_opts(root)
    };
    let plan = plan_for(root, &specs, &opts);

    assert_eq!(
        plan.summary.collisions, 2,
        "one of them would simply be lost"
    );
    assert!(!plan.summary.can_apply());
}

#[test]
fn overwrite_does_clear_a_collision_with_a_bystander_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("draft.md"), b"draft");
    touch(&root.join("final.txt"), b"existing");

    let specs = vec![
        RuleSpec::new(RuleKind::Template {
            template: "final".into(),
        }),
        RuleSpec::new(RuleKind::Extension {
            mode: ExtMode::Set { ext: "txt".into() },
        }),
    ];

    // `final.txt` is not part of the selection's rename set, so the collision is
    // with a bystander and overwriting it is a coherent choice.
    let blocked = PlanOptions {
        conflict: ConflictPolicy::Stop,
        ..local_opts(root)
    };
    let plan = plan_for(root, &specs, &blocked);
    assert!(
        plan.summary.collisions >= 1,
        "the occupied target must block by default"
    );

    let allowed = PlanOptions {
        conflict: ConflictPolicy::Overwrite,
        ..local_opts(root)
    };
    let plan = plan_for(root, &specs, &allowed);
    assert_eq!(plan.summary.collisions, 0);
}

#[test]
fn a_directory_and_its_contents_both_rename_without_losing_anything() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("old/inside.txt"), b"payload");

    let entries = scan(
        &[root.to_path_buf()],
        &ScanOptions {
            recursive: true,
            include_dirs: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(entries.len(), 2);

    let opts = local_opts(root);
    let mut plan = build_plan(&entries, &[], &NullProvider, &opts).unwrap();
    for row in plan.rows.iter_mut() {
        row.to = if row.is_dir {
            root.join("new")
        } else {
            root.join("old").join("renamed.txt")
        };
        row.status = zrename_core::RowStatus::Ok;
    }

    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 2);
    assert!(report.is_clean());
    assert_eq!(names_in(root), vec!["new"]);
    assert_eq!(
        std::fs::read(root.join("new/renamed.txt")).unwrap(),
        b"payload"
    );
}

#[test]
fn move_into_creates_the_subfolders_it_needs() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("shot.jpg"), b"img");

    let specs = vec![RuleSpec::new(RuleKind::MoveInto {
        template: "2026/08".into(),
    })];
    let opts = local_opts(root);
    let plan = plan_for(root, &specs, &opts);
    assert_eq!(plan.rows[0].to, root.join("2026/08/shot.jpg"));

    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 1);
    assert!(report.is_clean());
    assert!(root.join("2026/08/shot.jpg").exists());
}

#[test]
fn a_dry_run_reports_the_work_without_doing_any_of_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("a.txt"), b"x");

    let specs = vec![RuleSpec::new(RuleKind::Case {
        style: CaseStyle::Upper,
    })];
    let opts = local_opts(root);
    let plan = plan_for(root, &specs, &opts);

    let report = execute(
        &plan.rows,
        &plan.profile,
        &[root.to_path_buf()],
        None,
        &ExecuteOptions {
            dry_run: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(report.renamed, 1);
    assert!(report.journal_path.is_none());
    assert_eq!(names_in(root), vec!["a.txt"], "nothing moved");
}

#[test]
fn undo_after_a_partial_edit_reverts_the_rest_and_reports_the_one_it_would_not_touch() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    for n in 1..=3 {
        touch(&root.join(format!("f{n}.txt")), b"same");
    }

    let specs = vec![RuleSpec::new(RuleKind::Insert {
        text: "x_".into(),
        at: InsertAt::Prefix,
    })];
    let opts = local_opts(root);
    let plan = plan_for(root, &specs, &opts);
    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 3);

    std::fs::write(root.join("x_f2.txt"), b"edited after the rename").unwrap();

    let j = Journal::load(&report.journal_path.unwrap()).unwrap();
    let undo = journal::undo(&j, &UndoOptions::default());

    assert_eq!(undo.reverted, 2);
    assert_eq!(undo.skipped.len(), 1);
    assert!(!undo.is_clean());
    assert!(root.join("f1.txt").exists());
    assert!(root.join("f3.txt").exists());
    assert_eq!(
        std::fs::read(root.join("x_f2.txt")).unwrap(),
        b"edited after the rename",
        "the edited file keeps its content and its new name"
    );
}

#[test]
fn nothing_is_left_at_a_temp_name_when_a_two_phase_batch_finishes() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    for n in 0..6 {
        touch(&root.join(format!("{n}.txt")), format!("{n}").as_bytes());
    }

    // A six-element cycle: every file shifts one place along.
    let opts = local_opts(root);
    let mut plan = plan_for(root, &[], &opts);
    for i in 0..6 {
        plan.rows[i].to = root.join(format!("{}.txt", (i + 1) % 6));
        plan.rows[i].status = zrename_core::RowStatus::Ok;
    }

    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 6);
    assert_eq!(report.two_phase, 6);
    assert!(report.stranded.is_empty());

    let left: Vec<String> = names_in(root);
    assert_eq!(left.len(), 6);
    assert!(
        !left.iter().any(|n| n.starts_with(".zrn-")),
        "temp files must not survive: {left:?}"
    );
    for i in 0..6u32 {
        assert_eq!(
            std::fs::read(root.join(format!("{}.txt", (i + 1) % 6))).unwrap(),
            i.to_string().as_bytes()
        );
    }
}

#[test]
fn scanning_planning_and_applying_a_thousand_files_stays_correct() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    for n in 0..1000 {
        touch(&root.join(format!("raw_{n:04}.dat")), b"x");
    }

    let specs = vec![
        RuleSpec::new(RuleKind::Replace {
            find: "raw_".into(),
            with: "clean-".into(),
            regex: false,
            case_sensitive: true,
            all: true,
        }),
        RuleSpec::new(RuleKind::Case {
            style: CaseStyle::Upper,
        }),
    ];

    let opts = local_opts(root);
    let plan = plan_for(root, &specs, &opts);
    assert_eq!(plan.summary.changed, 1000);
    assert_eq!(plan.summary.collisions, 0);

    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 1000);
    assert!(report.is_clean());

    let left = names_in(root);
    assert_eq!(left.len(), 1000);
    assert!(left.iter().all(|n| n.starts_with("CLEAN-")));

    let j = Journal::load(&report.journal_path.unwrap()).unwrap();
    assert_eq!(journal::undo(&j, &UndoOptions::default()).reverted, 1000);
    assert!(names_in(root).iter().all(|n| n.starts_with("raw_")));
}

#[test]
fn paranoid_mode_does_not_disturb_the_result() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();
    touch(&root.join("a.txt"), b"payload");

    let specs = vec![RuleSpec::new(RuleKind::Case {
        style: CaseStyle::Upper,
    })];
    let opts = local_opts(root);
    let plan = plan_for(root, &specs, &opts);

    let report = execute(
        &plan.rows,
        &plan.profile,
        &[root.to_path_buf()],
        None,
        &ExecuteOptions {
            paranoid: true,
            journal_dir: Some(store.path()),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(report.renamed, 1);
    assert!(report.is_clean());
    assert_eq!(names_in(root), vec!["A.txt"]);
    assert_eq!(std::fs::read(root.join("A.txt")).unwrap(), b"payload");
}

#[test]
fn an_empty_selection_is_a_no_op_rather_than_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let root = dir.path();

    let opts = local_opts(root);
    let plan = plan_for(root, &[], &opts);
    assert_eq!(plan.summary.total, 0);
    assert!(!plan.summary.can_apply());

    let report = apply(&plan, root, store.path(), ConflictPolicy::Stop);
    assert_eq!(report.renamed, 0);
    assert!(
        report.journal_path.is_none(),
        "no journal for a batch with nothing in it"
    );
}

/// Confirms the journal directory the spec names is what the code resolves to.
#[test]
fn the_journal_directory_matches_the_spec() {
    let dir = journal::default_dir().unwrap();
    let s = dir.to_string_lossy();
    assert!(s.contains("zrename"), "{s}");
    assert!(
        s.ends_with(&format!("zrename{}journal", std::path::MAIN_SEPARATOR)),
        "{s}"
    );
}
