use super::{CompiledRule, RenameCtx};
use crate::error::Result;
use crate::model::{FsProfile, Scope};

pub struct SanitiseRule {
    pub illegal: bool,
    pub collapse_spaces: bool,
    pub transliterate: bool,
    pub replacement: String,
    pub trim_dots_spaces: bool,
    pub scope: Scope,
}

impl CompiledRule for SanitiseRule {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let fs = ctx.fs.clone();
        let opts = Options {
            illegal: self.illegal,
            collapse_spaces: self.collapse_spaces,
            transliterate: self.transliterate,
            replacement: &self.replacement,
            trim_dots_spaces: self.trim_dots_spaces,
        };
        ctx.map_scoped(self.scope, |s| sanitise(s, &fs, &opts));
        Ok(())
    }
}

pub struct Options<'a> {
    pub illegal: bool,
    pub collapse_spaces: bool,
    pub transliterate: bool,
    pub replacement: &'a str,
    pub trim_dots_spaces: bool,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            illegal: true,
            collapse_spaces: true,
            transliterate: false,
            replacement: "_",
            trim_dots_spaces: true,
        }
    }
}

pub fn sanitise(s: &str, fs: &FsProfile, opts: &Options) -> String {
    let mut out = if opts.transliterate {
        deunicode::deunicode(s)
    } else {
        s.to_string()
    };

    if opts.illegal {
        out = out
            .chars()
            .map(|c| {
                if fs.illegal_chars.contains(&c) {
                    opts.replacement.to_string()
                } else {
                    c.to_string()
                }
            })
            .collect();
    }

    if opts.collapse_spaces {
        out = collapse_runs(&out, ' ');
        if opts.replacement.len() == 1 {
            if let Some(r) = opts.replacement.chars().next() {
                if !r.is_alphanumeric() {
                    out = collapse_runs(&out, r);
                }
            }
        }
    }

    if opts.trim_dots_spaces {
        out = out
            .trim()
            .trim_end_matches(['.', ' '])
            .trim_start()
            .to_string();
    }

    out
}

fn collapse_runs(s: &str, c: char) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was = false;
    for ch in s.chars() {
        if ch == c {
            if last_was {
                continue;
            }
            last_was = true;
        } else {
            last_was = false;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ntfs(s: &str) -> String {
        sanitise(s, &FsProfile::ntfs(), &Options::default())
    }

    #[test]
    fn strips_characters_windows_forbids() {
        assert_eq!(ntfs("in<voice>:2024?.pdf"), "in_voice_2024_.pdf");
        assert_eq!(ntfs("a|b*c"), "a_b_c");
        assert_eq!(ntfs("back\\slash"), "back_slash");
    }

    #[test]
    fn leaves_posix_legal_names_alone_on_ext4() {
        let opts = Options::default();
        assert_eq!(
            sanitise("in<voice>:2024?.pdf", &FsProfile::ext4(), &opts),
            "in<voice>:2024?.pdf"
        );
        assert_eq!(sanitise("a/b", &FsProfile::ext4(), &opts), "a_b");
    }

    #[test]
    fn collapses_repeated_spaces_and_replacements() {
        assert_eq!(ntfs("scan   03"), "scan 03");
        assert_eq!(ntfs("a::::b"), "a_b");
    }

    #[test]
    fn trims_the_trailing_dots_and_spaces_windows_would_drop() {
        assert_eq!(ntfs("report."), "report");
        assert_eq!(ntfs("report .. "), "report");
        assert_eq!(ntfs("  leading"), "leading");
    }

    #[test]
    fn transliterates_only_when_asked() {
        let plain = Options::default();
        let translit = Options {
            transliterate: true,
            ..Options::default()
        };
        assert_eq!(
            sanitise("caf\u{e9}-r\u{e9}sum\u{e9}", &FsProfile::ntfs(), &plain),
            "caf\u{e9}-r\u{e9}sum\u{e9}"
        );
        assert_eq!(
            sanitise("caf\u{e9}-r\u{e9}sum\u{e9}", &FsProfile::ntfs(), &translit),
            "cafe-resume"
        );
        assert_eq!(
            sanitise("\u{6f22}\u{5b57}", &FsProfile::ntfs(), &translit),
            "Han Zi"
        );
    }

    #[test]
    fn the_usb_preset_case_from_the_spec() {
        let opts = Options {
            transliterate: true,
            ..Options::default()
        };
        let got = sanitise(
            "Urlaub 2024: M\u{fc}nchen <best>.JPG",
            &FsProfile::fat32(),
            &opts,
        );
        assert_eq!(got, "Urlaub 2024_ Munchen _best_.JPG");
        assert!(!got
            .chars()
            .any(|c| FsProfile::fat32().illegal_chars.contains(&c)));
    }

    #[test]
    fn a_name_of_only_illegal_characters_does_not_vanish_silently() {
        assert_eq!(ntfs("???"), "_");
    }

    #[test]
    fn control_characters_are_removed() {
        assert_eq!(ntfs("a\tb\nc"), "a_b_c");
    }
}
