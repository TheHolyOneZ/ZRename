use super::{CompiledRule, RenameCtx};
use crate::error::{CoreError, Result};
use regex::Regex;

const BASE: u32 = 0xF0000;
const LIMIT: u32 = 0xFFFD;

pub fn is_sentinel(c: char) -> bool {
    (BASE..BASE + LIMIT).contains(&(c as u32))
}

fn sentinel(i: usize) -> Option<char> {
    u32::try_from(i)
        .ok()
        .filter(|&i| i < LIMIT)
        .and_then(|i| char::from_u32(BASE + i))
}

pub fn compile_patterns(patterns: &[String]) -> Result<Option<Regex>> {
    let parts: Vec<String> = patterns
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty() && p.chars().any(|c| c != '*'))
        .map(glob_to_regex)
        .collect();
    if parts.is_empty() {
        return Ok(None);
    }
    let src = format!("(?i)(?:{})", parts.join("|"));
    Regex::new(&src)
        .map(Some)
        .map_err(|e| CoreError::Other(format!("leave-alone pattern: {e}")))
}

fn glob_to_regex(p: &str) -> String {
    let mut out = String::with_capacity(p.len() + 8);
    for c in p.chars() {
        match c {
            '*' => out.push_str(".*?"),
            '?' => out.push('.'),
            _ => out.push_str(&regex::escape(&c.to_string())),
        }
    }
    out
}

pub fn mask(s: &str, re: &Regex) -> Option<(String, Vec<String>)> {
    if s.chars().any(is_sentinel) {
        return None;
    }
    let mut out = String::with_capacity(s.len());
    let mut kept: Vec<String> = Vec::new();
    let mut last = 0;
    for m in re.find_iter(s) {
        if m.as_str().is_empty() {
            continue;
        }
        let mark = sentinel(kept.len())?;
        out.push_str(&s[last..m.start()]);
        out.push(mark);
        kept.push(m.as_str().to_string());
        last = m.end();
    }
    if kept.is_empty() {
        return None;
    }
    out.push_str(&s[last..]);
    Some((out, kept))
}

pub fn unmask(s: &str, kept: &[String]) -> Option<String> {
    let mut seen = vec![false; kept.len()];
    let mut out = String::with_capacity(s.len() + kept.iter().map(String::len).sum::<usize>());
    for c in s.chars() {
        if !is_sentinel(c) {
            out.push(c);
            continue;
        }
        let i = (c as u32 - BASE) as usize;
        let slot = seen.get_mut(i)?;
        if *slot {
            return None;
        }
        *slot = true;
        out.push_str(&kept[i]);
    }
    seen.iter().all(|&s| s).then_some(out)
}

pub struct Protected {
    pub inner: Box<dyn CompiledRule>,
    pub re: Regex,
}

impl CompiledRule for Protected {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        let Some((masked, kept)) = mask(&ctx.stem, &self.re) else {
            return self.inner.apply(ctx);
        };
        let before_stem = std::mem::replace(&mut ctx.stem, masked);
        let before_ext = ctx.ext.clone();
        self.inner.apply(ctx)?;
        match unmask(&ctx.stem, &kept) {
            Some(stem) => ctx.stem = stem,
            None => {
                ctx.stem = before_stem;
                ctx.ext = before_ext;
            }
        }
        Ok(())
    }

    fn needs_ordinals(&self) -> Option<&super::number::NumberParams> {
        self.inner.needs_ordinals()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn re(p: &[&str]) -> Regex {
        compile_patterns(&p.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            .unwrap()
            .unwrap()
    }

    #[test]
    fn wildcards_match_lazily_and_ignore_case() {
        let r = re(&["[*]"]);
        let (m, kept) = mask("[a] x [b]", &r).unwrap();
        assert_eq!(kept, vec!["[a]", "[b]"]);
        assert_eq!(m.chars().filter(|&c| is_sentinel(c)).count(), 2);

        let r = re(&["www.hbo.com"]);
        let (_, kept) = mask("[www.HBO.com]_x", &r).unwrap();
        assert_eq!(kept, vec!["www.HBO.com"]);
    }

    #[test]
    fn literal_characters_are_not_regex() {
        let r = re(&["a.b"]);
        assert!(mask("axb", &r).is_none());
        assert!(mask("a.b", &r).is_some());
    }

    #[test]
    fn empty_and_star_only_patterns_are_ignored() {
        let none: Vec<String> = vec!["".into(), "  ".into(), "*".into(), "**".into()];
        assert!(compile_patterns(&none).unwrap().is_none());
    }

    #[test]
    fn round_trips_and_rejects_lost_or_doubled_marks() {
        let r = re(&["[*]"]);
        let (m, kept) = mask("[keep]_rest", &r).unwrap();
        assert_eq!(unmask(&m, &kept).unwrap(), "[keep]_rest");
        assert!(unmask("_rest", &kept).is_none());
        assert!(unmask(&format!("{m}{m}"), &kept).is_none());
    }
}
