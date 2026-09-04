use super::{put, Values};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::prelude::{Accessor, ItemKey};
use lofty::probe::Probe;
use std::path::Path;

pub fn read(path: &Path) -> Values {
    let mut out = Values::new();
    let Ok(probe) = Probe::open(path) else {
        return out;
    };
    let Ok(tagged) = probe.read() else { return out };
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else {
        return out;
    };

    if let Some(v) = tag.artist() {
        put(&mut out, "artist", v.as_ref());
    }
    if let Some(v) = tag.title() {
        put(&mut out, "title", v.as_ref());
    }
    if let Some(v) = tag.album() {
        put(&mut out, "album", v.as_ref());
    }
    if let Some(v) = tag.genre() {
        put(&mut out, "genre", v.as_ref());
    }
    if let Some(v) = tag.comment() {
        put(&mut out, "comment", v.as_ref());
    }
    if let Some(v) = tag.track() {
        put(&mut out, "track", v.to_string());
        put(&mut out, "track2", format!("{v:02}"));
    }
    if let Some(v) = tag.track_total() {
        put(&mut out, "tracktotal", v.to_string());
    }
    if let Some(v) = tag.disk() {
        put(&mut out, "disc", v.to_string());
    }
    if let Some(ts) = tag.date() {
        put(&mut out, "year", ts.year.to_string());

        if let (Some(m), Some(d)) = (ts.month, ts.day) {
            put(&mut out, "date", format!("{:04}-{m:02}-{d:02}", ts.year));
        } else {
            put(&mut out, "date", ts.year.to_string());
        }
    }
    if let Some(v) = tag.get_string(ItemKey::AlbumArtist) {
        put(&mut out, "albumartist", v);
    }
    if let Some(v) = tag.get_string(ItemKey::Composer) {
        put(&mut out, "composer", v);
    }

    let props = tagged.properties();
    put(&mut out, "duration", props.duration().as_secs().to_string());
    if let Some(b) = props.audio_bitrate() {
        put(&mut out, "bitrate", b.to_string());
    }
    if let Some(r) = props.sample_rate() {
        put(&mut out, "samplerate", r.to_string());
    }
    if let Some(c) = props.channels() {
        put(&mut out, "channels", c.to_string());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_audio_yields_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.txt");
        std::fs::write(&p, b"plain text").unwrap();
        assert!(read(&p).is_empty());
    }

    #[test]
    fn a_missing_file_yields_nothing() {
        assert!(read(Path::new("/nonexistent.mp3")).is_empty());
    }
}
