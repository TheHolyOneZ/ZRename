use std::io::Read;
use std::path::Path;

pub fn compute(path: &Path, algo: &str) -> Option<String> {
    match algo.to_ascii_lowercase().as_str() {
        "crc32" | "crc" => crc32(path).map(|v| format!("{v:08x}")),
        "blake3" => blake3_short(path, 16),
        "blake3full" => blake3_short(path, 64),
        _ => None,
    }
}

fn crc32(path: &Path) -> Option<u32> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut hasher = crc32fast::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hasher.finalize())
}

fn blake3_short(path: &Path, chars: usize) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hasher.finalize().to_hex()[..chars].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(body: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f.bin");
        std::fs::write(&p, body).unwrap();
        (dir, p)
    }

    #[test]
    fn crc32_matches_the_known_value() {
        let (_d, p) = write(b"hello");
        assert_eq!(compute(&p, "crc32").unwrap(), "3610a686");
        assert_eq!(compute(&p, "CRC32").unwrap(), "3610a686");
    }

    #[test]
    fn different_content_gives_a_different_hash() {
        let (_d1, a) = write(b"hello");
        let (_d2, b) = write(b"world");
        assert_ne!(compute(&a, "crc32"), compute(&b, "crc32"));
        assert_ne!(compute(&a, "blake3"), compute(&b, "blake3"));
    }

    #[test]
    fn an_empty_file_still_hashes() {
        let (_d, p) = write(b"");
        assert_eq!(compute(&p, "crc32").unwrap(), "00000000");
        assert_eq!(compute(&p, "blake3").unwrap().len(), 16);
    }

    #[test]
    fn a_large_file_is_read_in_chunks() {
        let (_d, p) = write(&vec![7u8; 300 * 1024]);
        assert_eq!(compute(&p, "crc32").unwrap().len(), 8);
    }

    #[test]
    fn unknown_algorithms_and_missing_files_yield_nothing() {
        let (_d, p) = write(b"x");
        assert!(compute(&p, "sha1").is_none());
        assert!(compute(Path::new("/nonexistent/file"), "crc32").is_none());
    }
}
