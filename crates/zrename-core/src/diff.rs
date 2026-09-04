use crate::model::{DiffOp, DiffSpan};

const LCS_LIMIT: usize = 512;

pub fn diff(old: &str, new: &str) -> Vec<DiffSpan> {
    if old == new {
        return if old.is_empty() {
            Vec::new()
        } else {
            vec![span(DiffOp::Equal, old)]
        };
    }

    let a: Vec<char> = old.chars().collect();
    let b: Vec<char> = new.chars().collect();

    if a.len() > LCS_LIMIT || b.len() > LCS_LIMIT {
        return affix_diff(&a, &b);
    }
    coalesce(lcs_ops(&a, &b))
}

fn span(op: DiffOp, text: &str) -> DiffSpan {
    DiffSpan {
        op,
        text: text.to_string(),
    }
}

fn lcs_ops(a: &[char], b: &[char]) -> Vec<(DiffOp, char)> {
    let (n, m) = (a.len(), b.len());
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;

    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if a[i] == b[j] {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }

    let mut ops = Vec::with_capacity(n + m);
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            ops.push((DiffOp::Equal, a[i]));
            i += 1;
            j += 1;
        } else if table[at(i + 1, j)] >= table[at(i, j + 1)] {
            ops.push((DiffOp::Delete, a[i]));
            i += 1;
        } else {
            ops.push((DiffOp::Insert, b[j]));
            j += 1;
        }
    }
    ops.extend(a[i..].iter().map(|&c| (DiffOp::Delete, c)));
    ops.extend(b[j..].iter().map(|&c| (DiffOp::Insert, c)));
    ops
}

fn coalesce(ops: Vec<(DiffOp, char)>) -> Vec<DiffSpan> {
    let mut out: Vec<DiffSpan> = Vec::new();
    for (op, c) in ops {
        match out.last_mut() {
            Some(last) if last.op == op => last.text.push(c),
            _ => out.push(DiffSpan {
                op,
                text: c.to_string(),
            }),
        }
    }
    out
}

fn affix_diff(a: &[char], b: &[char]) -> Vec<DiffSpan> {
    let max = a.len().min(b.len());
    let mut pre = 0;
    while pre < max && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < max - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }

    let mut out = Vec::new();
    if pre > 0 {
        out.push(span(DiffOp::Equal, &a[..pre].iter().collect::<String>()));
    }
    let del: String = a[pre..a.len() - suf].iter().collect();
    let ins: String = b[pre..b.len() - suf].iter().collect();
    if !del.is_empty() {
        out.push(span(DiffOp::Delete, &del));
    }
    if !ins.is_empty() {
        out.push(span(DiffOp::Insert, &ins));
    }
    if suf > 0 {
        out.push(span(
            DiffOp::Equal,
            &a[a.len() - suf..].iter().collect::<String>(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(spans: &[DiffSpan]) -> String {
        spans
            .iter()
            .map(|s| match s.op {
                DiffOp::Equal => s.text.clone(),
                DiffOp::Insert => format!("[+{}]", s.text),
                DiffOp::Delete => format!("[-{}]", s.text),
            })
            .collect()
    }

    #[test]
    fn identical_names_are_one_equal_span() {
        assert_eq!(diff("a.txt", "a.txt"), vec![span(DiffOp::Equal, "a.txt")]);
        assert!(diff("", "").is_empty());
    }

    #[test]
    fn an_extension_case_change_shows_only_the_changed_letters() {
        assert_eq!(
            render(&diff("photo.JPG", "photo.jpg")),
            "photo.[-JPG][+jpg]"
        );
    }

    #[test]
    fn a_prefix_replacement_reads_cleanly() {
        assert_eq!(render(&diff("IMG_4821", "shot_4821")), "[-IMG][+shot]_4821");
    }

    #[test]
    fn pure_insertion_and_pure_deletion() {
        assert_eq!(render(&diff("report", "report.pdf")), "report[+.pdf]");
        assert_eq!(render(&diff("report.pdf", "report")), "report[-.pdf]");
        assert_eq!(render(&diff("", "new")), "[+new]");
        assert_eq!(render(&diff("old", "")), "[-old]");
    }

    #[test]
    fn a_double_space_is_visible_in_the_diff() {
        assert_eq!(render(&diff("scan  03", "scan 03")), "scan [- ]03");
    }

    #[test]
    fn the_spans_reconstruct_both_sides() {
        for (a, b) in [
            ("IMG_4821.JPG", "2026-08-14_01.jpg"),
            ("scan 03 (1).pdf", "scan-03.pdf"),
            ("\u{e4}\u{f6}\u{fc}.txt", "aou.txt"),
            ("a", "b"),
        ] {
            let spans = diff(a, b);
            let old: String = spans
                .iter()
                .filter(|s| s.op != DiffOp::Insert)
                .map(|s| s.text.as_str())
                .collect();
            let new: String = spans
                .iter()
                .filter(|s| s.op != DiffOp::Delete)
                .map(|s| s.text.as_str())
                .collect();
            assert_eq!(old, a, "delete+equal spans must rebuild the old name");
            assert_eq!(new, b, "insert+equal spans must rebuild the new name");
        }
    }

    #[test]
    fn very_long_names_fall_back_without_stalling() {
        let a = format!("{}middle{}", "x".repeat(600), "y".repeat(600));
        let b = format!("{}CENTER{}", "x".repeat(600), "y".repeat(600));
        let spans = diff(&a, &b);
        assert_eq!(
            render(&spans),
            format!("{}[-middle][+CENTER]{}", "x".repeat(600), "y".repeat(600))
        );
    }

    #[test]
    fn unicode_is_diffed_by_character_not_byte() {
        let spans = diff("caf\u{e9}.txt", "cafe.txt");
        let old: String = spans
            .iter()
            .filter(|s| s.op != DiffOp::Insert)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(old, "caf\u{e9}.txt");
    }
}
