pub mod case;
pub mod ext;
pub mod number;
pub mod sanitise;
pub mod text;

use crate::error::Result;
use crate::model::{FileEntry, FsProfile, MissingToken, RuleKind, RuleSpec, Scope};
use crate::tokens::{MetadataProvider, TokenCtx};
use std::path::PathBuf;

pub struct RenameCtx<'a> {
    pub stem: String,
    pub ext: Option<String>,

    pub subdir: Option<PathBuf>,
    pub entry: &'a FileEntry,

    pub counter: i64,
    pub index: usize,
    pub total: usize,
    pub placeholder: &'a str,
    pub on_missing: MissingToken,
    pub fs: &'a FsProfile,
    pub meta: &'a dyn MetadataProvider,

    pub skip: Option<String>,
}

impl<'a> RenameCtx<'a> {
    pub fn new(
        entry: &'a FileEntry,
        index: usize,
        total: usize,
        fs: &'a FsProfile,
        meta: &'a dyn MetadataProvider,
        placeholder: &'a str,
    ) -> Self {
        Self {
            stem: entry.stem.clone(),
            ext: entry.ext.clone(),
            subdir: None,
            entry,
            counter: index as i64 + 1,
            index,
            total,
            placeholder,
            on_missing: MissingToken::default(),
            fs,
            meta,
            skip: None,
        }
    }

    pub fn render_template(&mut self, segments: &[crate::tokens::Segment]) -> String {
        let out = crate::tokens::render_counted(segments, &self.token_ctx());
        if out.missing > 0 && self.on_missing == MissingToken::Skip {
            self.skip = Some(format!(
                "{} token{} had no value",
                out.missing,
                if out.missing == 1 { "" } else { "s" }
            ));
        }
        out.text
    }

    pub fn map_scoped(&mut self, scope: Scope, f: impl Fn(&str) -> String) {
        if scope.stem {
            self.stem = f(&self.stem);
        }
        if scope.ext {
            if let Some(e) = self.ext.take() {
                let mapped = f(&e);
                self.ext = (!mapped.is_empty()).then_some(mapped);
            }
        }
    }

    pub fn token_ctx(&self) -> TokenCtx<'_> {
        TokenCtx {
            entry: self.entry,
            counter: self.counter,
            index: self.index,
            total: self.total,
            placeholder: self.placeholder,
            meta: self.meta,
        }
    }

    pub fn file_name(&self) -> String {
        FileEntry::join_name(&self.stem, self.ext.as_deref())
    }
}

pub trait CompiledRule: Send + Sync {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()>;

    fn needs_ordinals(&self) -> Option<&number::NumberParams> {
        None
    }
}

pub fn compile(spec: &RuleSpec) -> Result<Box<dyn CompiledRule>> {
    let scope = spec.scope;
    Ok(match &spec.kind {
        RuleKind::Replace {
            find,
            with,
            regex,
            case_sensitive,
            all,
        } => Box::new(text::Replace::compile(
            find,
            with,
            *regex,
            *case_sensitive,
            *all,
            scope,
        )?),
        RuleKind::Case { style } => Box::new(case::CaseRule {
            style: *style,
            scope,
        }),
        RuleKind::Insert { text, at } => Box::new(text::Insert::compile(text, at.clone(), scope)),
        RuleKind::Remove { what } => Box::new(text::Remove {
            what: what.clone(),
            scope,
        }),
        RuleKind::Trim {
            whitespace,
            chars,
            collapse_spaces,
        } => Box::new(text::Trim {
            whitespace: *whitespace,
            chars: chars.chars().collect(),
            collapse_spaces: *collapse_spaces,
            scope,
        }),
        RuleKind::Number {
            start,
            step,
            pad,
            reset_per_folder,
            sort,
            descending,
            at,
        } => Box::new(number::NumberRule {
            params: number::NumberParams {
                start: *start,
                step: *step,
                reset_per_folder: *reset_per_folder,
                sort: *sort,
                descending: *descending,
            },
            pad: *pad,
            at: at.clone(),
            scope,
        }),
        RuleKind::Extension { mode } => Box::new(ext::ExtensionRule { mode: mode.clone() }),
        RuleKind::Sanitise {
            illegal,
            collapse_spaces,
            transliterate,
            replacement,
            trim_dots_spaces,
        } => Box::new(sanitise::SanitiseRule {
            illegal: *illegal,
            collapse_spaces: *collapse_spaces,
            transliterate: *transliterate,
            replacement: replacement.clone(),
            trim_dots_spaces: *trim_dots_spaces,
            scope,
        }),
        RuleKind::Template { template } => Box::new(text::Template::compile(template, scope)),
        RuleKind::MoveInto { template } => Box::new(text::MoveInto::compile(template)),
        RuleKind::CsvMap {
            path,
            match_full_name,
        } => Box::new(text::CsvMap::compile(path, *match_full_name)?),
    })
}

pub fn compile_stack(specs: &[RuleSpec]) -> Result<Vec<Box<dyn CompiledRule>>> {
    specs.iter().filter(|s| s.enabled).map(compile).collect()
}

pub fn split_words(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();

    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if !cur.is_empty() {
            let prev = chars[i - 1];
            let lower_to_upper = prev.is_lowercase() && c.is_uppercase();
            let digit_change = prev.is_numeric() != c.is_numeric();
            let acronym_end = prev.is_uppercase()
                && c.is_uppercase()
                && chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            if lower_to_upper || digit_change || acronym_end {
                words.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_separators() {
        assert_eq!(split_words("hello world"), vec!["hello", "world"]);
        assert_eq!(
            split_words("hello-world_foo.bar"),
            vec!["hello", "world", "foo", "bar"]
        );
        assert_eq!(split_words("  spaced  out  "), vec!["spaced", "out"]);
    }

    #[test]
    fn splits_on_case_transitions() {
        assert_eq!(split_words("helloWorld"), vec!["hello", "World"]);
        assert_eq!(split_words("HelloWorld"), vec!["Hello", "World"]);
        assert_eq!(split_words("HTTPServer"), vec!["HTTP", "Server"]);
        assert_eq!(split_words("myHTTPServer"), vec!["my", "HTTP", "Server"]);
    }

    #[test]
    fn splits_digits_from_letters() {
        assert_eq!(split_words("IMG_4821"), vec!["IMG", "4821"]);
        assert_eq!(split_words("S01E02"), vec!["S", "01", "E", "02"]);
    }

    #[test]
    fn empty_and_symbol_only_names() {
        assert!(split_words("").is_empty());
        assert!(split_words("---").is_empty());
    }
}
