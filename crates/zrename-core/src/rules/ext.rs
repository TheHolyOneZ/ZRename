use super::{CompiledRule, RenameCtx};
use crate::error::Result;
use crate::model::ExtMode;

pub struct ExtensionRule {
    pub mode: ExtMode,
}

impl CompiledRule for ExtensionRule {
    fn apply(&self, ctx: &mut RenameCtx) -> Result<()> {
        match &self.mode {
            ExtMode::Set { ext } => ctx.ext = normalise(ext),
            ExtMode::Lower => ctx.ext = ctx.ext.as_ref().map(|e| e.to_lowercase()),
            ExtMode::Upper => ctx.ext = ctx.ext.as_ref().map(|e| e.to_uppercase()),
            ExtMode::Remove => ctx.ext = None,
            ExtMode::Fill { ext } => {
                if ctx.ext.is_none() {
                    ctx.ext = normalise(ext);
                }
            }
        }
        Ok(())
    }
}

fn normalise(ext: &str) -> Option<String> {
    let e = ext.trim().trim_start_matches('.');
    (!e.is_empty()).then(|| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FileEntry, FsProfile};
    use crate::tokens::NullProvider;

    fn ctx_for(name: &str) -> (FileEntry, FsProfile) {
        let (stem, ext) = FileEntry::split_name(name);
        let e = FileEntry {
            path: std::path::PathBuf::from(format!("/tmp/{name}")),
            stem,
            ext,
            is_dir: false,
            is_symlink: false,
            size: 0,
            mtime: None,
            created: None,
            depth: 0,
        };
        (e, FsProfile::ext4())
    }

    fn run(name: &str, mode: ExtMode) -> String {
        let (entry, fs) = ctx_for(name);
        let meta = NullProvider;
        let mut ctx = RenameCtx::new(&entry, 0, 1, &fs, &meta, "_");
        ExtensionRule { mode }.apply(&mut ctx).unwrap();
        ctx.file_name()
    }

    #[test]
    fn sets_lowers_uppers_and_removes() {
        assert_eq!(
            run("photo.JPEG", ExtMode::Set { ext: "jpg".into() }),
            "photo.jpg"
        );
        assert_eq!(
            run("photo.JPEG", ExtMode::Set { ext: ".jpg".into() }),
            "photo.jpg"
        );
        assert_eq!(run("photo.JPG", ExtMode::Lower), "photo.jpg");
        assert_eq!(run("photo.jpg", ExtMode::Upper), "photo.JPG");
        assert_eq!(run("photo.jpg", ExtMode::Remove), "photo");
    }

    #[test]
    fn fill_only_touches_files_without_one() {
        assert_eq!(
            run("README", ExtMode::Fill { ext: "md".into() }),
            "README.md"
        );
        assert_eq!(
            run("notes.txt", ExtMode::Fill { ext: "md".into() }),
            "notes.txt"
        );
    }

    #[test]
    fn case_change_on_a_missing_extension_is_a_no_op() {
        assert_eq!(run("Makefile", ExtMode::Lower), "Makefile");
        assert_eq!(run("Makefile", ExtMode::Remove), "Makefile");
    }

    #[test]
    fn a_dotfile_keeps_its_leading_dot() {
        assert_eq!(run(".gitignore", ExtMode::Lower), ".gitignore");
        assert_eq!(
            run(".gitignore", ExtMode::Fill { ext: "txt".into() }),
            ".gitignore.txt"
        );
    }
}
