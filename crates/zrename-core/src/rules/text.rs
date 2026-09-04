use super::{CompiledRule, RenameCtx};
use crate::error::{CoreError, Result};
use crate::model::{FileEntry, InsertAt, RemoveWhat, Scope};
use crate::tokens::{self, Segment, TokenCtx};
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn render_for_regex(segments: &[Segment], ctx: &TokenCtx) -> String {
    let mut out = String::new();
    for seg in segments {
        match seg {
            Segment::Literal(s) => out.push_str(s),
            Segment::Token(_) => {
                let one = [seg.clone()];
                out.push_str(&tokens::render(&one, ctx).replace('$', "$$"));
            }
        }
    }
    out
}

enum Matcher {
    Regex(Regex),
    Literal(String),
}

pub struct Replace {
    matcher: Matcher,
    with: Vec<Segment>,
    all: bool,
    scope: Scope,
}

impl Replace {
    pub fn compile(
        find: &str,
        with: &str,
        regex: bool,
        case_sensitive: bool,
        all: bool,
        scope: Scope,
    ) -> Result<Self> {
        let matcher = match (regex, case_sensitive) {
            (true, true) => Matcher::Regex(Regex::new(find)?),
            (true, false) => Matcher::Regex(Regex::new(&format!("(?i){find}"))?),
            (false, true) => Matcher::Literal(find.to_string()),
            (false, false) => Matcher::Regex(Regex::new(&format!("(?i){}", regex::escape(find)))?),
        };
        Ok(Self {
            matcher,
            with: tokens::parse_template(with),
            all,
            scope,
        })
    }
}

impl CompiledRule for Replace {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let tctx = ctx.token_ctx();
        let is_regex = matches!(self.matcher, Matcher::Regex(_));
        let replacement = if is_regex {
            render_for_regex(&self.with, &tctx)
        } else {
            tokens::render(&self.with, &tctx)
        };

        let scope = self.scope;
        let all = self.all;
        match &self.matcher {
            Matcher::Regex(re) => {
                ctx.map_scoped(scope, |s| {
                    if all {
                        re.replace_all(s, replacement.as_str()).into_owned()
                    } else {
                        re.replace(s, replacement.as_str()).into_owned()
                    }
                });
            }
            Matcher::Literal(needle) => {
                if needle.is_empty() {
                    return Ok(());
                }
                ctx.map_scoped(scope, |s| {
                    if all {
                        s.replace(needle.as_str(), &replacement)
                    } else {
                        s.replacen(needle.as_str(), &replacement, 1)
                    }
                });
            }
        }
        Ok(())
    }
}

pub struct Insert {
    text: Vec<Segment>,
    at: InsertAt,
    scope: Scope,
}

impl Insert {
    pub fn compile(text: &str, at: InsertAt, scope: Scope) -> Self {
        Self {
            text: tokens::parse_template(text),
            at,
            scope,
        }
    }
}

impl CompiledRule for Insert {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let rendered = ctx.render_template(&self.text);
        let at = self.at.clone();
        ctx.map_scoped(self.scope, |s| insert_at(s, &rendered, &at));
        Ok(())
    }
}

pub fn insert_at(s: &str, text: &str, at: &InsertAt) -> String {
    match at {
        InsertAt::Prefix => format!("{text}{s}"),
        InsertAt::Suffix => format!("{s}{text}"),
        InsertAt::Index { index } => {
            let chars: Vec<char> = s.chars().collect();
            let pos = resolve_index(*index, chars.len());
            let head: String = chars[..pos].iter().collect();
            let tail: String = chars[pos..].iter().collect();
            format!("{head}{text}{tail}")
        }
        InsertAt::Before { marker, all } => splice_at_marker(s, text, marker, *all, false),
        InsertAt::After { marker, all } => splice_at_marker(s, text, marker, *all, true),
    }
}

fn splice_at_marker(s: &str, text: &str, marker: &str, all: bool, after: bool) -> String {
    if marker.is_empty() {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + text.len());
    let mut rest = s;
    while let Some(i) = rest.find(marker) {
        let (head, tail) = rest.split_at(i);
        out.push_str(head);
        if after {
            out.push_str(marker);
            out.push_str(text);
        } else {
            out.push_str(text);
            out.push_str(marker);
        }
        rest = &tail[marker.len()..];
        if !all {
            break;
        }
    }
    out.push_str(rest);
    out
}

pub fn resolve_index(index: i64, len: usize) -> usize {
    if index >= 0 {
        (index as usize).min(len)
    } else {
        let back = index.unsigned_abs() as usize;
        len.saturating_sub(back)
    }
}

pub struct Remove {
    pub what: RemoveWhat,
    pub scope: Scope,
}

impl CompiledRule for Remove {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let what = self.what.clone();
        ctx.map_scoped(self.scope, |s| remove(s, &what));
        Ok(())
    }
}

pub fn remove(s: &str, what: &RemoveWhat) -> String {
    match what {
        RemoveWhat::Range { from, to } => {
            let chars: Vec<char> = s.chars().collect();
            let a = resolve_index(*from, chars.len());
            let b = resolve_index(*to, chars.len());
            if a >= b {
                return s.to_string();
            }
            chars[..a].iter().chain(chars[b..].iter()).collect()
        }
        RemoveWhat::Chars { chars } => {
            let set: Vec<char> = chars.chars().collect();
            s.chars().filter(|c| !set.contains(c)).collect()
        }
        RemoveWhat::Word { word, all } => {
            if word.is_empty() {
                s.to_string()
            } else if *all {
                s.replace(word.as_str(), "")
            } else {
                s.replacen(word.as_str(), "", 1)
            }
        }
        RemoveWhat::Digits => s.chars().filter(|c| !c.is_numeric()).collect(),
        RemoveWhat::Duplicates { text } => {
            if text.is_empty() {
                return s.to_string();
            }
            let doubled = format!("{text}{text}");
            let mut out = s.to_string();
            while out.contains(&doubled) {
                out = out.replace(&doubled, text);
            }
            out
        }
    }
}

pub struct Trim {
    pub whitespace: bool,
    pub chars: Vec<char>,
    pub collapse_spaces: bool,
    pub scope: Scope,
}

impl CompiledRule for Trim {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let ws = self.whitespace;
        let set = self.chars.clone();
        let collapse = self.collapse_spaces;
        ctx.map_scoped(self.scope, |s| {
            let mut out = s.to_string();
            if collapse {
                out = collapse_whitespace(&out);
            }
            if !set.is_empty() {
                out = out.trim_matches(|c| set.contains(&c)).to_string();
            }
            if ws {
                out = out.trim().to_string();
            }
            out
        });
        Ok(())
    }
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !last_ws {
                out.push(' ');
            }
            last_ws = true;
        } else {
            out.push(c);
            last_ws = false;
        }
    }
    out
}

pub struct Template {
    segments: Vec<Segment>,
    scope: Scope,
}

impl Template {
    pub fn compile(template: &str, scope: Scope) -> Self {
        Self {
            segments: tokens::parse_template(template),
            scope,
        }
    }
}

impl CompiledRule for Template {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let rendered = ctx.render_template(&self.segments);
        if self.scope.stem && self.scope.ext {
            let (stem, ext) = FileEntry::split_name(&rendered);
            ctx.stem = stem;
            ctx.ext = ext;
        } else if self.scope.stem {
            ctx.stem = rendered;
        } else if self.scope.ext {
            ctx.ext = (!rendered.is_empty()).then_some(rendered);
        }
        Ok(())
    }
}

pub struct MoveInto {
    segments: Vec<Segment>,
}

impl MoveInto {
    pub fn compile(template: &str) -> Self {
        Self {
            segments: tokens::parse_template(template),
        }
    }
}

impl CompiledRule for MoveInto {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let rendered = ctx.render_template(&self.segments);
        let mut dir = PathBuf::new();
        for part in rendered.split(['/', '\\']) {
            let part = part.trim();
            if part.is_empty() || part == "." || part == ".." {
                continue;
            }
            dir.push(part);
        }
        ctx.subdir = (!dir.as_os_str().is_empty()).then_some(dir);
        Ok(())
    }
}

pub struct CsvMap {
    map: HashMap<String, String>,
    match_full_name: bool,
}

impl CsvMap {
    pub fn compile(path: &Path, match_full_name: bool) -> Result<Self> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(path)
            .map_err(|e| CoreError::Csv(format!("{}: {e}", path.display())))?;

        let mut map = HashMap::new();
        for (n, rec) in rdr.records().enumerate() {
            let rec = rec.map_err(|e| CoreError::Csv(format!("row {}: {e}", n + 1)))?;
            let old = rec.get(0).unwrap_or("").trim();
            let new = rec.get(1).unwrap_or("").trim();
            if old.is_empty() || new.is_empty() {
                continue;
            }
            if n == 0 && old.eq_ignore_ascii_case("old") {
                continue;
            }
            map.insert(old.to_string(), new.to_string());
        }
        Ok(Self {
            map,
            match_full_name,
        })
    }
}

impl CompiledRule for CsvMap {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let key = if self.match_full_name {
            ctx.file_name()
        } else {
            ctx.stem.clone()
        };
        let Some(new) = self.map.get(&key) else {
            return Ok(());
        };
        if self.match_full_name {
            let (stem, ext) = FileEntry::split_name(new);
            ctx.stem = stem;
            ctx.ext = ext;
        } else {
            ctx.stem = new.clone();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FsProfile;
    use crate::tokens::NullProvider;

    fn entry(name: &str) -> FileEntry {
        let (stem, ext) = FileEntry::split_name(name);
        FileEntry {
            path: PathBuf::from(format!("/tmp/import/{name}")),
            stem,
            ext,
            is_dir: false,
            is_symlink: false,
            size: 1234,
            mtime: None,
            created: None,
            depth: 0,
        }
    }

    fn run(name: &str, rule: &dyn CompiledRule) -> String {
        let e = entry(name);
        let fs = FsProfile::ext4();
        let meta = NullProvider;
        let mut ctx = RenameCtx::new(&e, 0, 1, &fs, &meta, "_");
        rule.apply(&mut ctx).unwrap();
        ctx.file_name()
    }

    #[test]
    fn plain_replace_is_case_insensitive_by_default() {
        let r = Replace::compile("img", "photo", false, false, true, Scope::STEM).unwrap();
        assert_eq!(run("IMG_4821.JPG", &r), "photo_4821.JPG");
    }

    #[test]
    fn plain_replace_can_be_case_sensitive() {
        let r = Replace::compile("img", "photo", false, true, true, Scope::STEM).unwrap();
        assert_eq!(run("IMG_4821.JPG", &r), "IMG_4821.JPG");
    }

    #[test]
    fn regex_replace_expands_capture_groups() {
        let r = Replace::compile("^IMG_(\\d+)", "photo-$1", true, true, true, Scope::STEM).unwrap();
        assert_eq!(run("IMG_4821.JPG", &r), "photo-4821.JPG");
    }

    #[test]
    fn regex_replace_honours_the_all_flag() {
        let first = Replace::compile("a", "X", true, true, false, Scope::STEM).unwrap();
        let every = Replace::compile("a", "X", true, true, true, Scope::STEM).unwrap();
        assert_eq!(run("banana.txt", &first), "bXnana.txt");
        assert_eq!(run("banana.txt", &every), "bXnXnX.txt");
    }

    #[test]
    fn replace_can_be_scoped_to_the_extension() {
        let r = Replace::compile("jpeg", "jpg", false, false, true, Scope::EXT).unwrap();
        assert_eq!(run("holiday.JPEG", &r), "holiday.jpg");
    }

    #[test]
    fn a_dollar_sign_in_a_plain_replacement_stays_literal() {
        let r = Replace::compile("cost", "$1000", false, true, true, Scope::STEM).unwrap();
        assert_eq!(run("cost.txt", &r), "$1000.txt");
    }

    #[test]
    fn an_invalid_regex_is_reported_at_compile_time() {
        assert!(Replace::compile("(unclosed", "x", true, true, true, Scope::STEM).is_err());
    }

    #[test]
    fn insert_at_every_position_form() {
        assert_eq!(insert_at("4821", "IMG_", &InsertAt::Prefix), "IMG_4821");
        assert_eq!(insert_at("4821", "_raw", &InsertAt::Suffix), "4821_raw");
        assert_eq!(
            insert_at("4821", "-", &InsertAt::Index { index: 2 }),
            "48-21"
        );
        assert_eq!(
            insert_at("4821", "-", &InsertAt::Index { index: -1 }),
            "482-1"
        );
        assert_eq!(
            insert_at("4821", "-", &InsertAt::Index { index: 99 }),
            "4821-"
        );
        assert_eq!(
            insert_at("4821", "-", &InsertAt::Index { index: -99 }),
            "-4821"
        );
    }

    #[test]
    fn insert_around_a_marker() {
        let before = InsertAt::Before {
            marker: "-".into(),
            all: false,
        };
        let after = InsertAt::After {
            marker: "-".into(),
            all: false,
        };
        let all = InsertAt::Before {
            marker: "-".into(),
            all: true,
        };
        assert_eq!(insert_at("a-b-c", "X", &before), "aX-b-c");
        assert_eq!(insert_at("a-b-c", "X", &after), "a-Xb-c");
        assert_eq!(insert_at("a-b-c", "X", &all), "aX-bX-c");
        assert_eq!(insert_at("abc", "X", &before), "abc");
    }

    #[test]
    fn insert_counts_characters_not_bytes() {
        assert_eq!(
            insert_at("\u{e4}\u{f6}\u{fc}", "-", &InsertAt::Index { index: 2 }),
            "\u{e4}\u{f6}-\u{fc}"
        );
    }

    #[test]
    fn remove_ranges_and_sets() {
        assert_eq!(
            remove("IMG_4821", &RemoveWhat::Range { from: 0, to: 4 }),
            "4821"
        );
        assert_eq!(
            remove("IMG_4821", &RemoveWhat::Range { from: -4, to: 99 }),
            "IMG_"
        );
        assert_eq!(
            remove("IMG_4821", &RemoveWhat::Range { from: 5, to: 2 }),
            "IMG_4821"
        );
        assert_eq!(
            remove("a-b_c", &RemoveWhat::Chars { chars: "-_".into() }),
            "abc"
        );
        assert_eq!(remove("scan 03 (1)", &RemoveWhat::Digits), "scan  ()");
        assert_eq!(
            remove(
                "a copy copy",
                &RemoveWhat::Word {
                    word: " copy".into(),
                    all: true
                }
            ),
            "a"
        );
        assert_eq!(
            remove(
                "a copy copy",
                &RemoveWhat::Word {
                    word: " copy".into(),
                    all: false
                }
            ),
            "a copy"
        );
        assert_eq!(
            remove("a__b____c", &RemoveWhat::Duplicates { text: "_".into() }),
            "a_b_c"
        );
    }

    #[test]
    fn trim_collapses_then_trims() {
        let t = Trim {
            whitespace: true,
            chars: vec![],
            collapse_spaces: true,
            scope: Scope::STEM,
        };
        assert_eq!(run("  scan   03  .pdf", &t), "scan 03.pdf");
    }

    #[test]
    fn trim_removes_a_custom_character_set_from_the_ends() {
        let t = Trim {
            whitespace: false,
            chars: vec!['_', '-'],
            collapse_spaces: false,
            scope: Scope::STEM,
        };
        assert_eq!(run("__report--.txt", &t), "report.txt");
    }

    #[test]
    fn a_full_name_template_resplits_the_extension() {
        let t = Template::compile("%file:stem%-copy.bak", Scope::BOTH);
        assert_eq!(run("notes.txt", &t), "notes-copy.bak");
    }

    #[test]
    fn a_stem_scoped_template_leaves_the_extension_alone() {
        let t = Template::compile("shot-%counter:3%", Scope::STEM);
        assert_eq!(run("IMG_4821.JPG", &t), "shot-001.JPG");
    }

    #[test]
    fn move_into_builds_a_relative_subdirectory() {
        let e = entry("a.jpg");
        let fs = FsProfile::ext4();
        let meta = NullProvider;
        let mut ctx = RenameCtx::new(&e, 0, 1, &fs, &meta, "_");
        MoveInto::compile("%folder:name%/sub")
            .apply(&mut ctx)
            .unwrap();
        assert_eq!(ctx.subdir.unwrap(), PathBuf::from("import/sub"));
    }

    #[test]
    fn move_into_refuses_to_escape_upwards() {
        let e = entry("a.jpg");
        let fs = FsProfile::ext4();
        let meta = NullProvider;

        let mut ctx = RenameCtx::new(&e, 0, 1, &fs, &meta, "_");
        MoveInto::compile("../../etc").apply(&mut ctx).unwrap();
        assert_eq!(ctx.subdir.unwrap(), PathBuf::from("etc"));

        let mut ctx = RenameCtx::new(&e, 0, 1, &fs, &meta, "_");
        MoveInto::compile("../..").apply(&mut ctx).unwrap();
        assert!(ctx.subdir.is_none());

        let mut ctx = RenameCtx::new(&e, 0, 1, &fs, &meta, "_");
        MoveInto::compile("/etc/passwd").apply(&mut ctx).unwrap();
        let sub = ctx.subdir.clone().unwrap();
        assert_eq!(sub, PathBuf::from("etc/passwd"));
        assert!(!sub.is_absolute());
    }

    #[test]
    fn csv_mapping_renames_only_listed_files() {
        let dir = tempfile::tempdir().unwrap();
        let csv_path = dir.path().join("map.csv");
        std::fs::write(&csv_path, "old,new\nIMG_4821,sunset\nIMG_4822,harbour\n").unwrap();
        let rule = CsvMap::compile(&csv_path, false).unwrap();
        assert_eq!(run("IMG_4821.JPG", &rule), "sunset.JPG");
        assert_eq!(run("IMG_9999.JPG", &rule), "IMG_9999.JPG");
    }

    #[test]
    fn csv_mapping_can_match_the_whole_name() {
        let dir = tempfile::tempdir().unwrap();
        let csv_path = dir.path().join("map.csv");
        std::fs::write(&csv_path, "IMG_4821.JPG,sunset.jpeg\n").unwrap();
        let rule = CsvMap::compile(&csv_path, true).unwrap();
        assert_eq!(run("IMG_4821.JPG", &rule), "sunset.jpeg");
    }
}
