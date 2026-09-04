use super::{put, Values};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn find_ffprobe() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

pub fn read(ffprobe: &Path, path: &Path) -> Values {
    let mut out = Values::new();
    let Ok(output) = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,codec_name,r_frame_rate:format=duration,bit_rate",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
    else {
        return out;
    };
    if !output.status.success() {
        return out;
    }

    let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return out;
    };
    parse_into(&json, &mut out);
    out
}

pub fn parse_into(json: &serde_json::Value, out: &mut Values) {
    if let Some(stream) = json.get("streams").and_then(|s| s.get(0)) {
        for (key, label) in [
            ("width", "width"),
            ("height", "height"),
            ("codec_name", "codec"),
        ] {
            if let Some(v) = scalar(stream.get(key)) {
                put(out, label, v);
            }
        }
        if let (Some(w), Some(h)) = (scalar(stream.get("width")), scalar(stream.get("height"))) {
            put(out, "resolution", format!("{w}x{h}"));
        }
        if let Some(rate) = stream.get("r_frame_rate").and_then(|v| v.as_str()) {
            if let Some(fps) = parse_rate(rate) {
                put(out, "fps", format!("{fps:.0}"));
            }
        }
    }

    if let Some(format) = json.get("format") {
        if let Some(d) = scalar(format.get("duration")) {
            if let Ok(secs) = d.parse::<f64>() {
                put(out, "duration", format!("{:.0}", secs));
                let total = secs as u64;
                put(
                    out,
                    "length",
                    format!(
                        "{:02}-{:02}-{:02}",
                        total / 3600,
                        (total % 3600) / 60,
                        total % 60
                    ),
                );
            }
        }
        if let Some(b) = scalar(format.get("bit_rate")) {
            put(out, "bitrate", b);
        }
    }
}

fn scalar(v: Option<&serde_json::Value>) -> Option<String> {
    match v? {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn parse_rate(s: &str) -> Option<f64> {
    let (num, den) = s.split_once('/')?;
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    (den != 0.0).then(|| num / den)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_rate_fractions_convert() {
        assert_eq!(parse_rate("30/1").unwrap().round(), 30.0);
        assert_eq!(parse_rate("30000/1001").unwrap().round(), 30.0);
        assert!(parse_rate("25").is_none());
        assert!(parse_rate("1/0").is_none());
    }

    #[test]
    fn a_typical_ffprobe_response_is_parsed() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{
              "streams": [{"width": 1920, "height": 1080, "codec_name": "h264", "r_frame_rate": "30000/1001"}],
              "format": {"duration": "3723.5", "bit_rate": "8000000"}
            }"#,
        )
        .unwrap();

        let mut out = Values::new();
        parse_into(&json, &mut out);

        assert_eq!(super::super::lookup(&out, "width").unwrap(), "1920");
        assert_eq!(super::super::lookup(&out, "height").unwrap(), "1080");
        assert_eq!(
            super::super::lookup(&out, "resolution").unwrap(),
            "1920x1080"
        );
        assert_eq!(super::super::lookup(&out, "codec").unwrap(), "h264");
        assert_eq!(super::super::lookup(&out, "fps").unwrap(), "30");
        assert_eq!(super::super::lookup(&out, "duration").unwrap(), "3724");
        assert_eq!(super::super::lookup(&out, "length").unwrap(), "01-02-03");
        assert_eq!(super::super::lookup(&out, "bitrate").unwrap(), "8000000");
    }

    #[test]
    fn an_audio_only_file_reports_no_video_stream() {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"streams": [], "format": {"duration": "10"}}"#).unwrap();
        let mut out = Values::new();
        parse_into(&json, &mut out);
        assert!(super::super::lookup(&out, "width").is_none());
        assert_eq!(super::super::lookup(&out, "duration").unwrap(), "10");
    }

    #[test]
    fn empty_output_yields_nothing() {
        let mut out = Values::new();
        parse_into(&serde_json::json!({}), &mut out);
        assert!(out.is_empty());
    }
}
