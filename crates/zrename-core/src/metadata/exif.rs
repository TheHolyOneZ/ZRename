use super::{put, Values};
use std::path::Path;

pub fn read(path: &Path) -> Values {
    let mut out = Values::new();
    let Ok(file) = std::fs::File::open(path) else {
        return out;
    };
    let mut reader = std::io::BufReader::new(file);
    let Ok(exif) = ::exif::Reader::new().read_from_container(&mut reader) else {
        return out;
    };

    for field in exif.fields() {
        let name = format!("{}", field.tag);
        let name = name.rsplit(' ').next().unwrap_or(&name).to_string();
        let value = field.display_value().with_unit(&exif).to_string();
        put(&mut out, &name, clean(&value));
    }
    out
}

fn clean(v: &str) -> String {
    v.trim().trim_matches('"').trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_an_image_yields_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, b"plain text").unwrap();
        assert!(read(&p).is_empty());
    }

    #[test]
    fn a_missing_file_yields_nothing() {
        assert!(read(Path::new("/nonexistent.jpg")).is_empty());
    }

    #[test]
    fn display_values_are_unquoted_and_trimmed() {
        assert_eq!(clean("\"Canon EOS R6\""), "Canon EOS R6");
        assert_eq!(clean("  2026:08:14 10:23:45  "), "2026:08:14 10:23:45");
    }
}
