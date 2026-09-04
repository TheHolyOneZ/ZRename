use crate::error::{CoreError, Result};
use crate::execute::ConflictPolicy;
use crate::model::{CaseStyle, ExtMode, InsertAt, RuleKind, RuleSpec, Scope, SortKey};
use crate::scan::ScanOptions;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<ConflictPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan: Option<ScanOptions>,
    #[serde(default)]
    pub rules: Vec<RuleSpec>,
}

impl Preset {
    pub fn new(name: impl Into<String>, rules: Vec<RuleSpec>) -> Self {
        Self {
            name: name.into(),
            description: None,
            conflict: None,
            scan: None,
            rules,
        }
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| CoreError::Preset(format!("serialising: {e}")))
    }

    pub fn from_toml(text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|e| CoreError::Preset(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|e| CoreError::io(path, e))?;
        Self::from_toml(&text).map_err(|e| CoreError::Preset(format!("{}: {e}", path.display())))
    }

    pub fn save(&self, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir).map_err(|e| CoreError::io(dir, e))?;
        let path = dir.join(format!("{}.toml", slug(&self.name)));
        std::fs::write(&path, self.to_toml()?).map_err(|e| CoreError::io(&path, e))?;
        Ok(path)
    }
}

pub fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = false;
    for c in deunicode::deunicode(name).chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-').to_string();
    if trimmed.is_empty() {
        "preset".to_string()
    } else {
        trimmed
    }
}

pub fn default_dir() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| CoreError::Preset("no config directory on this system".into()))?;
    Ok(base.join("zrename").join("presets"))
}

pub fn list(dir: &Path) -> Vec<Preset> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Preset> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
        .filter_map(|p| Preset::load(&p).ok())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn install_starters(dir: &Path) -> Result<usize> {
    std::fs::create_dir_all(dir).map_err(|e| CoreError::io(dir, e))?;
    let mut written = 0;
    for p in starters() {
        let path = dir.join(format!("{}.toml", slug(&p.name)));
        if path.exists() {
            continue;
        }
        std::fs::write(&path, p.to_toml()?).map_err(|e| CoreError::io(&path, e))?;
        written += 1;
    }
    Ok(written)
}

pub fn starters() -> Vec<Preset> {
    vec![photos_by_date(), tv_episodes(), sanitise_for_usb()]
}

fn photos_by_date() -> Preset {
    let mut p = Preset::new(
        "Photos \u{2192} date-based",
        vec![
            RuleSpec::new(RuleKind::Template {
                template: "%exif:DateTimeOriginal:%Y-%m-%d%".into(),
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
            RuleSpec::new(RuleKind::Insert {
                text: "_".into(),
                at: InsertAt::Index { index: 10 },
            }),
            RuleSpec::new(RuleKind::Extension {
                mode: ExtMode::Lower,
            }),
        ],
    );
    p.description =
        Some("Names each photo after the day it was taken, numbered within its folder.".into());
    p.scan = Some(ScanOptions {
        extensions: vec![
            "jpg".into(),
            "jpeg".into(),
            "png".into(),
            "heic".into(),
            "dng".into(),
        ],
        ..Default::default()
    });
    p
}

fn tv_episodes() -> Preset {
    let mut p = Preset::new(
        "TV episodes \u{2192} S01E02",
        vec![
            RuleSpec::new(RuleKind::Replace {
                find: r"[Ss](\d{1,2})[\s._-]*[Ee](\d{1,2})".into(),
                with: "S${1}E${2}".into(),
                regex: true,
                case_sensitive: false,
                all: false,
            }),
            RuleSpec::new(RuleKind::Replace {
                find: r"S(\d)E".into(),
                with: "S0${1}E".into(),
                regex: true,
                case_sensitive: true,
                all: false,
            }),
            RuleSpec::new(RuleKind::Replace {
                find: r"E(\d)(\D|$)".into(),
                with: "E0${1}${2}".into(),
                regex: true,
                case_sensitive: true,
                all: false,
            }),
            RuleSpec::new(RuleKind::Replace {
                find: r"[._]+".into(),
                with: " ".into(),
                regex: true,
                case_sensitive: true,
                all: true,
            }),
            RuleSpec::new(RuleKind::Trim {
                whitespace: true,
                chars: String::new(),
                collapse_spaces: true,
            }),
        ],
    );
    p.description =
        Some("Normalises season and episode markers, and tidies dot-separated names.".into());
    p.scan = Some(ScanOptions {
        extensions: vec!["mkv".into(), "mp4".into(), "avi".into(), "srt".into()],
        ..Default::default()
    });
    p
}

fn sanitise_for_usb() -> Preset {
    let mut p = Preset::new(
        "Sanitise for USB/FAT32",
        vec![
            RuleSpec::new(RuleKind::Sanitise {
                illegal: true,
                collapse_spaces: true,
                transliterate: true,
                replacement: "_".into(),
                trim_dots_spaces: true,
            })
            .with_scope(Scope::BOTH),
            RuleSpec::new(RuleKind::Case {
                style: CaseStyle::Lower,
            })
            .with_scope(Scope::EXT),
        ],
    );
    p.description = Some(
        "Strips characters Windows rejects and transliterates accents, for a stick that has to be readable everywhere."
            .into(),
    );
    p.conflict = Some(ConflictPolicy::Suffix);
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preset_survives_a_toml_round_trip() {
        for original in starters() {
            let text = original.to_toml().unwrap();
            let back = Preset::from_toml(&text)
                .unwrap_or_else(|e| panic!("{} failed to parse back: {e}\n{text}", original.name));

            assert_eq!(back.name, original.name);
            assert_eq!(back.rules.len(), original.rules.len(), "{}", original.name);
            for (a, b) in back.rules.iter().zip(original.rules.iter()) {
                assert_eq!(a.kind, b.kind, "rule mismatch in {}", original.name);
                assert_eq!(a.scope, b.scope);
                assert_eq!(a.enabled, b.enabled);
            }
        }
    }

    #[test]
    fn every_rule_kind_survives_a_toml_round_trip() {
        let rules = vec![
            RuleSpec::new(RuleKind::Replace {
                find: "^IMG_(\\d+)".into(),
                with: "%exif:DateTimeOriginal:%Y-%m-%d%_".into(),
                regex: true,
                case_sensitive: false,
                all: true,
            }),
            RuleSpec::new(RuleKind::Case {
                style: CaseStyle::Title,
            }),
            RuleSpec::new(RuleKind::Insert {
                text: "x".into(),
                at: InsertAt::Before {
                    marker: "-".into(),
                    all: true,
                },
            }),
            RuleSpec::new(RuleKind::Insert {
                text: "y".into(),
                at: InsertAt::Index { index: -2 },
            }),
            RuleSpec::new(RuleKind::Remove {
                what: crate::model::RemoveWhat::Range { from: 0, to: 4 },
            }),
            RuleSpec::new(RuleKind::Remove {
                what: crate::model::RemoveWhat::Digits,
            }),
            RuleSpec::new(RuleKind::Trim {
                whitespace: true,
                chars: "_-".into(),
                collapse_spaces: true,
            }),
            RuleSpec::new(RuleKind::Number {
                start: 5,
                step: 2,
                pad: 3,
                reset_per_folder: true,
                sort: SortKey::Size,
                descending: true,
                at: InsertAt::Prefix,
            }),
            RuleSpec::new(RuleKind::Extension {
                mode: ExtMode::Set { ext: "jpg".into() },
            }),
            RuleSpec::new(RuleKind::Extension {
                mode: ExtMode::Lower,
            }),
            RuleSpec::new(RuleKind::Sanitise {
                illegal: true,
                collapse_spaces: true,
                transliterate: true,
                replacement: "-".into(),
                trim_dots_spaces: true,
            }),
            RuleSpec::new(RuleKind::Template {
                template: "%counter:3%".into(),
            }),
            RuleSpec::new(RuleKind::MoveInto {
                template: "%exif:Y%/%exif:m%".into(),
            }),
            RuleSpec::new(RuleKind::CsvMap {
                path: PathBuf::from("/tmp/map.csv"),
                match_full_name: true,
            }),
        ];

        let preset = Preset::new("everything", rules.clone());
        let text = preset.to_toml().unwrap();
        let back = Preset::from_toml(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));

        assert_eq!(back.rules.len(), rules.len());
        for (a, b) in back.rules.iter().zip(rules.iter()) {
            assert_eq!(a.kind, b.kind, "{:?} did not survive TOML", b.kind);
        }
    }

    #[test]
    fn the_hand_written_form_from_the_spec_parses() {
        let text = r#"
name = "Photos -> date-based"

[[rules]]
kind = "sanitise"

[[rules]]
kind = "replace"
find = "^IMG_(\\d+)"
with = "%exif:DateTimeOriginal:%Y-%m-%d%_"
regex = true

[[rules]]
kind = "number"
start = 1
pad = 2
reset_per_folder = true
"#;
        let p = Preset::from_toml(text).unwrap();
        assert_eq!(p.name, "Photos -> date-based");
        assert_eq!(p.rules.len(), 3);

        assert!(matches!(p.rules[0].kind, RuleKind::Sanitise { .. }));
        assert!(p.rules[0].enabled, "omitted fields take sensible defaults");

        match &p.rules[1].kind {
            RuleKind::Replace {
                find,
                regex,
                case_sensitive,
                ..
            } => {
                assert_eq!(find, "^IMG_(\\d+)");
                assert!(regex);
                assert!(!case_sensitive, "case sensitivity is off unless asked for");
            }
            other => panic!("expected a replace rule, got {other:?}"),
        }

        match &p.rules[2].kind {
            RuleKind::Number {
                start,
                pad,
                step,
                reset_per_folder,
                ..
            } => {
                assert_eq!(*start, 1);
                assert_eq!(*pad, 2);
                assert_eq!(*step, 1, "step defaults to 1");
                assert!(reset_per_folder);
            }
            other => panic!("expected a number rule, got {other:?}"),
        }
    }

    #[test]
    fn presets_save_and_load_from_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        for p in starters() {
            p.save(dir.path()).unwrap();
        }
        let loaded = list(dir.path());
        assert_eq!(loaded.len(), 3);
        assert!(loaded.iter().any(|p| p.name.contains("Photos")));
        assert!(loaded.iter().any(|p| p.name.contains("S01E02")));
        assert!(loaded.iter().any(|p| p.name.contains("USB")));
    }

    #[test]
    fn installing_starters_never_overwrites_an_edited_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(install_starters(dir.path()).unwrap(), 3);
        assert_eq!(
            install_starters(dir.path()).unwrap(),
            0,
            "second run must change nothing"
        );

        let path = dir
            .path()
            .join(format!("{}.toml", slug("Sanitise for USB/FAT32")));
        std::fs::write(&path, "name = \"mine\"\n").unwrap();
        assert_eq!(install_starters(dir.path()).unwrap(), 0);
        assert_eq!(Preset::load(&path).unwrap().name, "mine");
    }

    #[test]
    fn a_broken_preset_does_not_hide_the_others() {
        let dir = tempfile::tempdir().unwrap();
        Preset::new("good", vec![]).save(dir.path()).unwrap();
        std::fs::write(dir.path().join("bad.toml"), "this is not = valid = toml").unwrap();
        let loaded = list(dir.path());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "good");
    }

    #[test]
    fn slugs_are_safe_filenames() {
        assert_eq!(slug("Photos \u{2192} date-based"), "photos-date-based");
        assert_eq!(slug("Sanitise for USB/FAT32"), "sanitise-for-usb-fat32");
        assert_eq!(slug("TV episodes \u{2192} S01E02"), "tv-episodes-s01e02");
        assert_eq!(slug("M\u{fc}nchen"), "munchen");
        assert_eq!(slug("///"), "preset");
        assert_eq!(slug(""), "preset");
        for name in ["Photos \u{2192} date-based", "a:b?c*d", "CON"] {
            let s = slug(name);
            assert!(!s.chars().any(|c| "<>:\"/\\|?*".contains(c)), "{s}");
        }
    }

    #[test]
    fn the_starter_presets_actually_run() {
        use crate::model::{FileEntry, FsProfile};
        use crate::plan::{compute_targets, PlanOptions};
        use crate::tokens::NullProvider;

        let entry = FileEntry {
            path: PathBuf::from("/a/some.file.mkv"),
            stem: "some.file".into(),
            ext: Some("mkv".into()),
            is_dir: false,
            is_symlink: false,
            size: 1,
            mtime: None,
            created: None,
            depth: 1,
        };
        let opts = PlanOptions {
            profile: FsProfile::ext4(),
            ..Default::default()
        };

        for preset in starters() {
            let rows = compute_targets(
                std::slice::from_ref(&entry),
                &preset.rules,
                &NullProvider,
                &opts,
            )
            .unwrap_or_else(|e| panic!("{} failed to compile: {e}", preset.name));
            assert_eq!(rows.len(), 1, "{}", preset.name);
        }
    }

    #[test]
    fn the_tv_preset_normalises_episode_markers() {
        use crate::model::{FileEntry, FsProfile};
        use crate::plan::{compute_targets, PlanOptions};
        use crate::tokens::NullProvider;

        let make = |name: &str| {
            let (stem, ext) = FileEntry::split_name(name);
            FileEntry {
                path: PathBuf::from(format!("/a/{name}")),
                stem,
                ext,
                is_dir: false,
                is_symlink: false,
                size: 1,
                mtime: None,
                created: None,
                depth: 1,
            }
        };

        let files = [
            make("Show.Name.s01.e02.Pilot.mkv"),
            make("Show Name S1E2 Pilot.mkv"),
        ];
        let opts = PlanOptions {
            profile: FsProfile::ext4(),
            ..Default::default()
        };
        let rows = compute_targets(&files, &tv_episodes().rules, &NullProvider, &opts).unwrap();

        let names: Vec<String> = rows.iter().map(|r| r.to_name()).collect();
        assert_eq!(names[0], "Show Name S01E02 Pilot.mkv");
        assert_eq!(
            names[1], "Show Name S01E02 Pilot.mkv",
            "the preset is named after its output, so it must pad to two digits"
        );
    }
}
