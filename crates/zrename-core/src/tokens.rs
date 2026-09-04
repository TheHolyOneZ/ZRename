use crate::model::FileEntry;
use chrono::{DateTime, Local};
use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Segment {
    Literal(String),
    Token(Token),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub ns: String,
    pub key: String,
    pub fmt: Option<String>,
}

impl Token {
    fn render_source(&self) -> String {
        let mut s = format!("%{}", self.ns);
        if !self.key.is_empty() {
            s.push(':');
            s.push_str(&self.key);
        }
        if let Some(f) = &self.fmt {
            s.push(':');
            s.push_str(f);
        }
        s.push('%');
        s
    }
}

pub trait MetadataProvider: Send + Sync {
    fn resolve(&self, entry: &FileEntry, ns: &str, key: &str, fmt: Option<&str>) -> Option<String>;
}

pub struct NullProvider;

impl MetadataProvider for NullProvider {
    fn resolve(&self, _e: &FileEntry, _ns: &str, _k: &str, _f: Option<&str>) -> Option<String> {
        None
    }
}

pub struct TokenCtx<'a> {
    pub entry: &'a FileEntry,
    pub counter: i64,
    pub index: usize,
    pub total: usize,
    pub placeholder: &'a str,
    pub meta: &'a dyn MetadataProvider,
}

const LOCAL_NAMESPACES: &[&str] = &["counter", "file", "folder", "date", "now", "index"];

pub fn parse_template(input: &str) -> Vec<Segment> {
    let chars: Vec<char> = input.chars().collect();
    let mut out: Vec<Segment> = Vec::new();
    let mut literal = String::new();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] != '%' {
            literal.push(chars[i]);
            i += 1;
            continue;
        }
        if chars.get(i + 1) == Some(&'%') {
            literal.push('%');
            i += 2;
            continue;
        }
        match parse_token(&chars, i) {
            Some((token, next)) => {
                if !literal.is_empty() {
                    out.push(Segment::Literal(std::mem::take(&mut literal)));
                }
                out.push(Segment::Token(token));
                i = next;
            }
            None => {
                literal.push('%');
                i += 1;
            }
        }
    }
    if !literal.is_empty() {
        out.push(Segment::Literal(literal));
    }
    out
}

fn parse_token(chars: &[char], start: usize) -> Option<(Token, usize)> {
    let mut i = start + 1;
    let ns = read_segment(chars, &mut i)?;
    if ns.is_empty() {
        return None;
    }

    if chars.get(i) == Some(&'%') {
        return Some((
            Token {
                ns,
                key: String::new(),
                fmt: None,
            },
            i + 1,
        ));
    }
    i += 1;

    if chars.get(i) == Some(&'{') {
        let (fmt, next) = read_braced(chars, i)?;
        return Some((
            Token {
                ns,
                key: String::new(),
                fmt: Some(fmt),
            },
            next,
        ));
    }

    let key = read_segment(chars, &mut i)?;
    match chars.get(i) {
        Some('%') => Some((Token { ns, key, fmt: None }, i + 1)),
        Some('{') => {
            let (fmt, next) = read_braced(chars, i)?;
            Some((
                Token {
                    ns,
                    key,
                    fmt: Some(fmt),
                },
                next,
            ))
        }
        Some(':') => {
            let (fmt, next) = read_format(chars, i + 1)?;
            let fmt = (!fmt.is_empty()).then_some(fmt);
            Some((Token { ns, key, fmt }, next))
        }
        _ => None,
    }
}

fn read_segment(chars: &[char], i: &mut usize) -> Option<String> {
    let begin = *i;
    while let Some(&c) = chars.get(*i) {
        if c == ':' || c == '%' || c == '{' {
            return Some(chars[begin..*i].iter().collect());
        }
        *i += 1;
    }
    None
}

fn read_braced(chars: &[char], open: usize) -> Option<(String, usize)> {
    let mut i = open + 1;
    while let Some(&c) = chars.get(i) {
        if c == '}' {
            if chars.get(i + 1) != Some(&'%') {
                return None;
            }
            return Some((chars[open + 1..i].iter().collect(), i + 2));
        }
        i += 1;
    }
    None
}

fn read_format(chars: &[char], begin: usize) -> Option<(String, usize)> {
    let mut i = begin;
    let mut fallback: Option<usize> = None;

    while i < chars.len() {
        if chars[i] != '%' {
            i += 1;
            continue;
        }
        match chars.get(i + 1) {
            Some(&c) if c.is_ascii_alphabetic() || c == '%' || c == ':' => {
                if i > begin {
                    fallback = Some(i);
                }
                i += 2;
            }
            _ => return Some((chars[begin..i].iter().collect(), i + 1)),
        }
    }

    let close = fallback?;
    Some((chars[begin..close].iter().collect(), close + 1))
}

pub struct Rendered {
    pub text: String,

    pub missing: usize,
}

pub fn render(segments: &[Segment], ctx: &TokenCtx) -> String {
    render_counted(segments, ctx).text
}

pub fn render_counted(segments: &[Segment], ctx: &TokenCtx) -> Rendered {
    let mut out = String::new();
    let mut missing = 0;
    for seg in segments {
        match seg {
            Segment::Literal(s) => out.push_str(s),
            Segment::Token(t) => match resolve_token(t, ctx) {
                Resolved::Value(v) => out.push_str(&v),
                Resolved::Missing => {
                    missing += 1;
                    out.push_str(ctx.placeholder);
                }
                Resolved::UnknownNamespace => out.push_str(&t.render_source()),
            },
        }
    }
    Rendered { text: out, missing }
}

enum Resolved {
    Value(String),
    Missing,
    UnknownNamespace,
}

fn resolve_token(t: &Token, ctx: &TokenCtx) -> Resolved {
    let ns = t.ns.to_ascii_lowercase();
    if LOCAL_NAMESPACES.contains(&ns.as_str()) {
        return match resolve_local(&ns, t, ctx) {
            Some(v) => Resolved::Value(v),
            None => Resolved::Missing,
        };
    }
    if !is_known_namespace(&ns) {
        return Resolved::UnknownNamespace;
    }
    match ctx.meta.resolve(ctx.entry, &ns, &t.key, t.fmt.as_deref()) {
        Some(v) => Resolved::Value(v),
        None => Resolved::Missing,
    }
}

pub fn is_known_namespace(ns: &str) -> bool {
    LOCAL_NAMESPACES.contains(&ns)
        || matches!(ns, "exif" | "id3" | "audio" | "video" | "pdf" | "hash")
}

fn resolve_local(ns: &str, t: &Token, ctx: &TokenCtx) -> Option<String> {
    let e = ctx.entry;
    match ns {
        "counter" => {
            let pad = t.key.parse::<usize>().unwrap_or(0);
            Some(pad_number(ctx.counter, pad))
        }
        "index" => {
            let pad = t.key.parse::<usize>().unwrap_or(0);
            Some(pad_number(ctx.index as i64, pad))
        }
        "file" => match t.key.to_ascii_lowercase().as_str() {
            "name" => Some(e.file_name()),
            "stem" | "base" | "" => Some(e.stem.clone()),
            "ext" => e.ext.clone(),
            "size" => Some(e.size.to_string()),
            "created" => e.created.and_then(|s| format_time(s, t.fmt.as_deref())),
            "modified" | "mtime" => e.mtime.and_then(|s| format_time(s, t.fmt.as_deref())),
            "total" => Some(ctx.total.to_string()),
            _ => None,
        },
        "folder" => {
            let parent = e.path.parent()?;
            match t.key.to_ascii_lowercase().as_str() {
                "name" | "" => parent.file_name().map(|n| n.to_string_lossy().into_owned()),
                "path" => Some(parent.to_string_lossy().into_owned()),
                "parent" => parent
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned()),
                _ => None,
            }
        }
        "date" | "now" => {
            let fmt = t.fmt.clone().or_else(|| shorthand_date_format(&t.key));
            format_time(SystemTime::now(), fmt.as_deref())
        }
        _ => None,
    }
}

fn pad_number(n: i64, pad: usize) -> String {
    if n < 0 {
        format!("-{:0>width$}", n.unsigned_abs(), width = pad)
    } else {
        format!("{n:0>pad$}")
    }
}

pub fn format_time(t: SystemTime, fmt: Option<&str>) -> Option<String> {
    let dt: DateTime<Local> = t.into();
    let fmt = fmt.unwrap_or("%Y-%m-%d");
    Some(dt.format(fmt).to_string())
}

pub fn shorthand_date_format(key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    let mut out = String::new();
    let mut saw_letter = false;
    for c in key.chars() {
        match c {
            'Y' | 'y' | 'm' | 'd' | 'H' | 'M' | 'S' | 'j' | 'b' | 'B' | 'a' | 'A' => {
                out.push('%');
                out.push(c);
                saw_letter = true;
            }
            '-' | '_' | '.' | ' ' => out.push(c),
            _ => return None,
        }
    }
    saw_letter.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(ns: &str, key: &str, fmt: Option<&str>) -> Segment {
        Segment::Token(Token {
            ns: ns.into(),
            key: key.into(),
            fmt: fmt.map(String::from),
        })
    }

    fn lit(s: &str) -> Segment {
        Segment::Literal(s.into())
    }

    #[test]
    fn parses_the_spec_example_with_a_nested_strftime_format() {
        assert_eq!(
            parse_template("%exif:DateTimeOriginal:%Y-%m-%d%_"),
            vec![tok("exif", "DateTimeOriginal", Some("%Y-%m-%d")), lit("_")]
        );
    }

    #[test]
    fn parses_two_segment_tokens() {
        assert_eq!(
            parse_template("%exif:Model%"),
            vec![tok("exif", "Model", None)]
        );
        assert_eq!(
            parse_template("%counter:3%"),
            vec![tok("counter", "3", None)]
        );
        assert_eq!(
            parse_template("%hash:crc32%"),
            vec![tok("hash", "crc32", None)]
        );
        assert_eq!(
            parse_template("%id3:artist%"),
            vec![tok("id3", "artist", None)]
        );
        assert_eq!(
            parse_template("%video:width%"),
            vec![tok("video", "width", None)]
        );
    }

    #[test]
    fn parses_single_segment_tokens() {
        assert_eq!(parse_template("%counter%"), vec![tok("counter", "", None)]);
    }

    #[test]
    fn format_ends_at_end_of_input() {
        assert_eq!(
            parse_template("%file:created:%Y%m%d%"),
            vec![tok("file", "created", Some("%Y%m%d"))]
        );
    }

    #[test]
    fn backtracks_when_a_literal_letter_follows_the_closing_percent() {
        assert_eq!(
            parse_template("%file:created:%Y-%m-%d%final"),
            vec![tok("file", "created", Some("%Y-%m-%d")), lit("final")]
        );
    }

    #[test]
    fn brace_form_is_unambiguous() {
        assert_eq!(
            parse_template("%exif:DateTimeOriginal{%Y-%m-%d}%x"),
            vec![tok("exif", "DateTimeOriginal", Some("%Y-%m-%d")), lit("x")]
        );
        assert_eq!(
            parse_template("%file:modified{%Y_%H%M%S}%"),
            vec![tok("file", "modified", Some("%Y_%H%M%S"))]
        );
    }

    #[test]
    fn double_percent_is_a_literal() {
        assert_eq!(parse_template("100%% done"), vec![lit("100% done")]);
        assert_eq!(parse_template("%%"), vec![lit("%")]);
    }

    #[test]
    fn unterminated_tokens_survive_as_literals() {
        assert_eq!(parse_template("%exif:Model"), vec![lit("%exif:Model")]);
        assert_eq!(parse_template("50% off"), vec![lit("50% off")]);
        assert_eq!(parse_template("%"), vec![lit("%")]);
        assert_eq!(parse_template("%:%"), vec![lit("%:%")]);
    }

    #[test]
    fn mixes_literals_and_several_tokens() {
        assert_eq!(
            parse_template("IMG_%exif:DateTimeOriginal:%Y-%m-%d%_%counter:3%.jpg"),
            vec![
                lit("IMG_"),
                tok("exif", "DateTimeOriginal", Some("%Y-%m-%d")),
                lit("_"),
                tok("counter", "3", None),
                lit(".jpg"),
            ]
        );
    }

    #[test]
    fn colon_specifier_stays_inside_the_format() {
        assert_eq!(
            parse_template("%file:modified:%Y%:z%"),
            vec![tok("file", "modified", Some("%Y%:z"))]
        );
    }

    #[test]
    fn shorthand_date_only_matches_date_letters() {
        assert_eq!(shorthand_date_format("Y-m-d").unwrap(), "%Y-%m-%d");
        assert_eq!(shorthand_date_format("Ymd").unwrap(), "%Y%m%d");
        assert_eq!(shorthand_date_format("Y_m").unwrap(), "%Y_%m");
        assert!(shorthand_date_format("Model").is_none());
        assert!(shorthand_date_format("DateTimeOriginal").is_none());
        assert!(shorthand_date_format("").is_none());
        assert!(shorthand_date_format("---").is_none());
    }

    #[test]
    fn pads_numbers_including_negatives() {
        assert_eq!(pad_number(7, 3), "007");
        assert_eq!(pad_number(1234, 3), "1234");
        assert_eq!(pad_number(0, 0), "0");
        assert_eq!(pad_number(-4, 3), "-004");
    }

    fn entry(path: &str) -> FileEntry {
        let p = std::path::PathBuf::from(path);
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let (stem, ext) = FileEntry::split_name(&name);
        FileEntry {
            path: p,
            stem,
            ext,
            is_dir: false,
            is_symlink: false,
            size: 2048,
            mtime: None,
            created: None,
            depth: 0,
        }
    }

    #[test]
    fn renders_local_namespaces() {
        let e = entry("/home/z/Pictures/import/IMG_4821.JPG");
        let ctx = TokenCtx {
            entry: &e,
            counter: 7,
            index: 3,
            total: 12,
            placeholder: "_",
            meta: &NullProvider,
        };
        let r = |s: &str| render(&parse_template(s), &ctx);
        assert_eq!(r("%file:stem%"), "IMG_4821");
        assert_eq!(r("%file:ext%"), "JPG");
        assert_eq!(r("%file:name%"), "IMG_4821.JPG");
        assert_eq!(r("%file:size%"), "2048");
        assert_eq!(r("%folder:name%"), "import");
        assert_eq!(r("%folder:parent%"), "Pictures");
        assert_eq!(r("%counter:3%"), "007");
        assert_eq!(r("%index:2%"), "03");
        assert_eq!(r("%file:total%"), "12");
    }

    #[test]
    fn missing_values_become_the_placeholder() {
        let e = entry("/tmp/a.txt");
        let ctx = TokenCtx {
            entry: &e,
            counter: 1,
            index: 0,
            total: 1,
            placeholder: "NA",
            meta: &NullProvider,
        };
        assert_eq!(render(&parse_template("%exif:Model%"), &ctx), "NA");
        assert_eq!(render(&parse_template("x-%id3:artist%-y"), &ctx), "x-NA-y");
    }

    #[test]
    fn unknown_namespaces_stay_visible() {
        let e = entry("/tmp/a.txt");
        let ctx = TokenCtx {
            entry: &e,
            counter: 1,
            index: 0,
            total: 1,
            placeholder: "NA",
            meta: &NullProvider,
        };
        assert_eq!(render(&parse_template("%nope:key%"), &ctx), "%nope:key%");
    }

    #[test]
    fn a_template_of_pure_literal_round_trips() {
        let e = entry("/tmp/a.txt");
        let ctx = TokenCtx {
            entry: &e,
            counter: 1,
            index: 0,
            total: 1,
            placeholder: "_",
            meta: &NullProvider,
        };
        assert_eq!(
            render(&parse_template("holiday photos"), &ctx),
            "holiday photos"
        );
    }
}
