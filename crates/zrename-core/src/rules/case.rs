use super::{split_words, CompiledRule, RenameCtx};
use crate::error::Result;
use crate::model::{CaseStyle, Scope};

pub struct CaseRule {
    pub style: CaseStyle,
    pub scope: Scope,
}

impl CompiledRule for CaseRule {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let style = self.style;
        ctx.map_scoped(self.scope, |s| convert(s, style));
        Ok(())
    }
}

pub fn convert(s: &str, style: CaseStyle) -> String {
    match style {
        CaseStyle::Lower => s.to_lowercase(),
        CaseStyle::Upper => s.to_uppercase(),
        CaseStyle::Title => title(s),
        CaseStyle::Sentence => sentence(s),
        CaseStyle::Camel => camel(s, false),
        CaseStyle::Pascal => camel(s, true),
        CaseStyle::Snake => join_words(s, '_'),
        CaseStyle::Kebab => join_words(s, '-'),
    }
}

fn title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_boundary = true;
    for c in s.chars() {
        if at_boundary {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
        at_boundary = !c.is_alphanumeric();
    }
    out
}

fn sentence(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

fn camel(s: &str, first_upper: bool) -> String {
    let words = split_words(s);
    let mut out = String::with_capacity(s.len());
    for (i, w) in words.iter().enumerate() {
        let mut chars = w.chars();
        let Some(first) = chars.next() else { continue };
        if i == 0 && !first_upper {
            out.extend(first.to_lowercase());
        } else {
            out.extend(first.to_uppercase());
        }
        out.extend(chars.flat_map(|c| c.to_lowercase()));
    }
    out
}

fn join_words(s: &str, sep: char) -> String {
    split_words(s)
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join(&sep.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use CaseStyle::*;

    fn c(s: &str, style: CaseStyle) -> String {
        convert(s, style)
    }

    #[test]
    fn the_full_style_matrix() {
        let s = "my HTTP server_file 02";
        assert_eq!(c(s, Lower), "my http server_file 02");
        assert_eq!(c(s, Upper), "MY HTTP SERVER_FILE 02");
        assert_eq!(c(s, Title), "My Http Server_File 02");
        assert_eq!(c(s, Sentence), "My http server_file 02");
        assert_eq!(c(s, Camel), "myHttpServerFile02");
        assert_eq!(c(s, Pascal), "MyHttpServerFile02");
        assert_eq!(c(s, Snake), "my_http_server_file_02");
        assert_eq!(c(s, Kebab), "my-http-server-file-02");
    }

    #[test]
    fn title_preserves_separators_and_position() {
        assert_eq!(c("hello world-foo.bar", Title), "Hello World-Foo.Bar");
        assert_eq!(c("a b", Title).len(), "a b".len());
    }

    #[test]
    fn sentence_only_lifts_the_first_letter() {
        assert_eq!(c("hELLO WORLD", Sentence), "Hello world");
        assert_eq!(c("2001 a space odyssey", Sentence), "2001 a space odyssey");
    }

    #[test]
    fn photo_names_convert_sensibly() {
        assert_eq!(c("IMG_4821", Snake), "img_4821");
        assert_eq!(c("IMG_4821", Kebab), "img-4821");
        assert_eq!(c("IMG_4821", Camel), "img4821");
        assert_eq!(c("IMG_4821", Lower), "img_4821");
    }

    #[test]
    fn handles_unicode_without_panicking() {
        assert_eq!(c("\u{e4}\u{f6}\u{fc}", Upper), "\u{c4}\u{d6}\u{dc}");
        assert_eq!(c("\u{c4}\u{d6}\u{dc}", Lower), "\u{e4}\u{f6}\u{fc}");
        assert_eq!(c("\u{e9}t\u{e9} photo", Title), "\u{c9}t\u{e9} Photo");
    }

    #[test]
    fn empty_input_stays_empty() {
        for style in [Lower, Upper, Title, Sentence, Camel, Pascal, Snake, Kebab] {
            assert_eq!(c("", style), "");
        }
    }
}
