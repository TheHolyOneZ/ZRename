use super::{put, Values};
use lopdf::{Document, Object};
use std::path::Path;

pub fn read(path: &Path) -> Values {
    let mut out = Values::new();
    let Ok(doc) = Document::load(path) else {
        return out;
    };

    put(&mut out, "pages", doc.get_pages().len().to_string());

    let Ok(info) = doc.trailer.get(b"Info") else {
        return out;
    };
    let dict = match info {
        Object::Reference(id) => match doc.get_object(*id).and_then(|o| o.as_dict().cloned()) {
            Ok(d) => d,
            Err(_) => return out,
        },
        Object::Dictionary(d) => d.clone(),
        _ => return out,
    };

    for (key, label) in [
        (&b"Title"[..], "title"),
        (&b"Author"[..], "author"),
        (&b"Subject"[..], "subject"),
        (&b"Keywords"[..], "keywords"),
        (&b"Creator"[..], "creator"),
        (&b"Producer"[..], "producer"),
        (&b"CreationDate"[..], "creationdate"),
        (&b"ModDate"[..], "moddate"),
    ] {
        if let Ok(obj) = dict.get(key) {
            if let Some(text) = decode(obj) {
                put(&mut out, label, text);
            }
        }
    }
    out
}

fn decode(obj: &Object) -> Option<String> {
    let bytes = obj.as_str().ok()?;
    let text = if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };

    let text = text.trim().trim_start_matches("D:").trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_a_pdf_yields_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, b"plain text").unwrap();
        assert!(read(&p).is_empty());
    }

    #[test]
    fn utf16_strings_with_a_byte_order_mark_decode() {
        let mut bytes = vec![0xFE, 0xFF];
        for u in "Report".encode_utf16() {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
        assert_eq!(
            decode(&Object::String(bytes, lopdf::StringFormat::Literal)).unwrap(),
            "Report"
        );
    }

    #[test]
    fn plain_strings_decode_and_dates_lose_their_prefix() {
        let s = |b: &[u8]| Object::String(b.to_vec(), lopdf::StringFormat::Literal);
        assert_eq!(decode(&s(b"Quarterly Report")).unwrap(), "Quarterly Report");
        assert_eq!(decode(&s(b"D:20260814102345")).unwrap(), "20260814102345");
        assert!(decode(&s(b"   ")).is_none());
    }
}
