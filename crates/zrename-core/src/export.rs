use crate::error::{CoreError, Result};
use crate::model::{Plan, RowStatus};

pub fn status_label(status: &RowStatus) -> String {
    match status {
        RowStatus::Ok => "ok".into(),
        RowStatus::Unchanged => "unchanged".into(),
        RowStatus::Collision { existing: true, .. } => "collision (target exists)".into(),
        RowStatus::Collision { with, .. } => format!("collision with {} other row(s)", with.len()),
        RowStatus::Invalid { reason } => format!("invalid: {reason}"),
        RowStatus::TooLong { limit, actual, .. } => format!("too long: {actual} of {limit}"),
        RowStatus::ReservedName { name } => format!("reserved name: {name}"),
        RowStatus::Skipped { reason } => format!("skipped: {reason}"),
    }
}

pub fn to_csv(plan: &Plan) -> Result<String> {
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record(["old_name", "new_name", "status", "old_path", "new_path"])
        .map_err(|e| CoreError::Csv(e.to_string()))?;

    for row in &plan.rows {
        w.write_record([
            row.from_name(),
            row.to_name(),
            status_label(&row.status),
            row.from.to_string_lossy().into_owned(),
            row.to.to_string_lossy().into_owned(),
        ])
        .map_err(|e| CoreError::Csv(e.to_string()))?;
    }

    let bytes = w.into_inner().map_err(|e| CoreError::Csv(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| CoreError::Csv(e.to_string()))
}

pub fn to_markdown(plan: &Plan) -> String {
    let s = &plan.summary;
    let mut out = String::new();
    out.push_str("# ZRename plan\n\n");
    out.push_str(&format!(
        "{} file(s) · {} will change · {} unchanged · {} skipped\n",
        s.total, s.changed, s.unchanged, s.skipped
    ));
    if s.blocking() > 0 {
        out.push_str(&format!(
            "\n**{} row(s) block Apply**: {} collision(s), {} invalid, {} too long, {} reserved.\n",
            s.blocking(),
            s.collisions,
            s.invalid,
            s.too_long,
            s.reserved
        ));
    }
    out.push_str(&format!("\nTarget filesystem: `{}`\n\n", plan.profile.name));

    out.push_str("| old | new | status |\n|---|---|---|\n");
    for row in &plan.rows {
        out.push_str(&format!(
            "| `{}` | `{}` | {} |\n",
            escape_cell(&row.from_name()),
            escape_cell(&row.to_name()),
            escape_cell(&status_label(&row.status))
        ));
    }
    out
}

fn escape_cell(s: &str) -> String {
    s.replace('|', "\\|")
}

pub fn summary_line(plan: &Plan) -> String {
    let s = &plan.summary;
    let mut parts = vec![format!("{} will change", s.changed)];
    if s.collisions > 0 {
        parts.push(format!(
            "{} collision{}",
            s.collisions,
            plural(s.collisions)
        ));
    }
    if s.invalid > 0 {
        parts.push(format!("{} invalid", s.invalid));
    }
    if s.too_long > 0 {
        parts.push(format!("{} too long", s.too_long));
    }
    if s.reserved > 0 {
        parts.push(format!("{} reserved", s.reserved));
    }
    if s.skipped > 0 {
        parts.push(format!("{} skipped", s.skipped));
    }
    parts.join(" \u{b7} ")
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

pub fn apply_label(plan: &Plan) -> String {
    match plan.summary.changed {
        1 => "Apply 1 rename".into(),
        n => format!("Apply {n} renames"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FsProfile, PlanRow, PlanSummary};
    use std::path::PathBuf;

    fn row(i: usize, from: &str, to: &str, status: RowStatus) -> PlanRow {
        PlanRow {
            index: i,
            from: PathBuf::from(from),
            to: PathBuf::from(to),
            status,
            is_dir: false,
            is_symlink: false,
            case_only: false,
        }
    }

    fn plan_of(rows: Vec<PlanRow>) -> Plan {
        let summary = crate::validate::summarise(&rows);
        Plan {
            rows,
            summary,
            profile: FsProfile::ext4(),
        }
    }

    #[test]
    fn csv_has_a_header_and_one_row_per_file() {
        let p = plan_of(vec![
            row(0, "/a/IMG_1.JPG", "/a/shot.jpg", RowStatus::Ok),
            row(1, "/a/keep.txt", "/a/keep.txt", RowStatus::Unchanged),
        ]);
        let csv = to_csv(&p).unwrap();
        let lines: Vec<&str> = csv.trim().lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("old_name,new_name,status"));
        assert!(lines[1].starts_with("IMG_1.JPG,shot.jpg,ok"));
        assert!(lines[2].contains("unchanged"));
    }

    #[test]
    fn a_name_with_a_comma_or_a_quote_stays_intact() {
        let p = plan_of(vec![row(
            0,
            "/a/Smith, John.pdf",
            "/a/say \"hi\".pdf",
            RowStatus::Ok,
        )]);
        let csv = to_csv(&p).unwrap();
        let mut rdr = csv::Reader::from_reader(csv.as_bytes());
        let rec = rdr.records().next().unwrap().unwrap();
        assert_eq!(&rec[0], "Smith, John.pdf");
        assert_eq!(&rec[1], "say \"hi\".pdf");
    }

    #[test]
    fn markdown_reports_the_blocking_rows() {
        let p = plan_of(vec![
            row(
                0,
                "/a/a.txt",
                "/a/same.txt",
                RowStatus::Collision {
                    with: vec![1],
                    existing: false,
                },
            ),
            row(
                1,
                "/a/b.txt",
                "/a/same.txt",
                RowStatus::Collision {
                    with: vec![0],
                    existing: false,
                },
            ),
        ]);
        let md = to_markdown(&p);
        assert!(md.contains("block Apply"));
        assert!(md.contains("2 collision(s)"));
        assert!(md.contains("| `a.txt` | `same.txt` |"));
        assert!(md.contains("ext4"));
    }

    #[test]
    fn a_pipe_in_a_filename_does_not_break_the_table() {
        let p = plan_of(vec![row(0, "/a/we|rd.txt", "/a/fine.txt", RowStatus::Ok)]);
        let md = to_markdown(&p);
        assert!(md.contains("we\\|rd.txt"));
        let table_row = md.lines().find(|l| l.contains("fine.txt")).unwrap();
        assert_eq!(
            table_row.matches("| ").count(),
            3,
            "the row must still have three cells"
        );
    }

    #[test]
    fn the_summary_line_reads_like_the_spec_mockup() {
        let mut p = plan_of(vec![]);
        p.summary = PlanSummary {
            total: 1284,
            changed: 1281,
            collisions: 1,
            skipped: 2,
            ..Default::default()
        };
        assert_eq!(
            summary_line(&p),
            "1281 will change \u{b7} 1 collision \u{b7} 2 skipped"
        );
    }

    #[test]
    fn the_apply_button_states_the_number() {
        let mut p = plan_of(vec![]);
        p.summary.changed = 1281;
        assert_eq!(apply_label(&p), "Apply 1281 renames");
        p.summary.changed = 1;
        assert_eq!(apply_label(&p), "Apply 1 rename");
        p.summary.changed = 0;
        assert_eq!(apply_label(&p), "Apply 0 renames");
    }

    #[test]
    fn every_status_gets_a_readable_label() {
        let cases = [
            RowStatus::Ok,
            RowStatus::Unchanged,
            RowStatus::Collision {
                with: vec![1],
                existing: false,
            },
            RowStatus::Collision {
                with: vec![],
                existing: true,
            },
            RowStatus::Invalid {
                reason: "`?` is not allowed".into(),
            },
            RowStatus::TooLong {
                limit: 255,
                actual: 400,
                unit: crate::model::LengthUnit::Bytes,
            },
            RowStatus::ReservedName { name: "CON".into() },
            RowStatus::Skipped {
                reason: "hidden".into(),
            },
        ];
        for s in cases {
            let label = status_label(&s);
            assert!(!label.is_empty());
            assert!(!label.contains("RowStatus"), "{label} leaks the type name");
        }
    }

    #[test]
    fn an_empty_plan_exports_cleanly() {
        let p = plan_of(vec![]);
        assert!(to_csv(&p).unwrap().contains("old_name"));
        assert!(to_markdown(&p).contains("ZRename plan"));
        assert_eq!(summary_line(&p), "0 will change");
    }
}
