//! `rsepuh verify` and `rsepuh check`: prove a render is real (parse the MP4, decode PNG frames)
//! and test it against the upload limits of each platform. No ffmpeg needed.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub struct Finding {
    pub file: String,
    pub ok: bool,
    pub detail: String,
}

// ------------------------------------------------------------------ MP4

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Video {
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub frames: u64,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Mp4 {
    pub bytes: u64,
    pub duration: f64,
    pub video: Option<Video>,
    pub audio: Option<String>,
    pub fps: f64,
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn be64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// Iterate the boxes of `b`: (type, content).
fn boxes(b: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 8 <= b.len() {
        let size32 = be32(b, at).unwrap_or(0) as u64;
        let ty: [u8; 4] = b[at + 4..at + 8].try_into().unwrap();
        let (hdr, size) = match size32 {
            1 => (16usize, be64(b, at + 8).unwrap_or(0)),
            0 => (8usize, (b.len() - at) as u64),
            n => (8usize, n),
        };
        if size < hdr as u64 || at as u64 + size > b.len() as u64 {
            break;
        }
        out.push((ty, &b[at + hdr..at + size as usize]));
        at += size as usize;
    }
    out
}

fn find<'a>(b: &'a [u8], ty: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(b).into_iter().find(|(t, _)| t == ty).map(|(_, c)| c)
}

pub fn parse_mp4(b: &[u8]) -> Result<Mp4, String> {
    let top = boxes(b);
    if !top.iter().any(|(t, _)| t == b"ftyp") {
        return Err("no ftyp box: not an MP4".into());
    }
    let moov = top
        .iter()
        .find(|(t, _)| t == b"moov")
        .map(|(_, c)| *c)
        .ok_or("no moov box (file is truncated or not finalized)")?;
    let mut info = Mp4 {
        bytes: b.len() as u64,
        ..Default::default()
    };
    if let Some(mvhd) = find(moov, b"mvhd") {
        let (ts, dur) = if mvhd.first() == Some(&1) {
            (be32(mvhd, 20), be64(mvhd, 24))
        } else {
            (be32(mvhd, 12), be32(mvhd, 16).map(u64::from))
        };
        if let (Some(ts), Some(dur)) = (ts, dur) {
            if ts > 0 {
                info.duration = dur as f64 / ts as f64;
            }
        }
    }
    for (t, trak) in boxes(moov) {
        if &t != b"trak" {
            continue;
        }
        let Some(mdia) = find(trak, b"mdia") else {
            continue;
        };
        let handler = find(mdia, b"hdlr")
            .and_then(|h| h.get(8..12))
            .map(|h| h.to_vec())
            .unwrap_or_default();
        let stbl = find(mdia, b"minf").and_then(|m| find(m, b"stbl"));
        let codec = stbl
            .and_then(|s| find(s, b"stsd"))
            .and_then(|d| d.get(12..16))
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .unwrap_or_default();
        match handler.as_slice() {
            b"vide" => {
                let (mut w, mut h) = (0, 0);
                if let Some(tkhd) = find(trak, b"tkhd") {
                    let n = tkhd.len();
                    if n >= 8 {
                        w = be32(tkhd, n - 8).unwrap_or(0) >> 16;
                        h = be32(tkhd, n - 4).unwrap_or(0) >> 16;
                    }
                }
                let frames = stbl
                    .and_then(|s| find(s, b"stsz"))
                    .and_then(|z| be32(z, 8))
                    .unwrap_or(0) as u64;
                let mdur = find(mdia, b"mdhd").and_then(|m| {
                    if m.first() == Some(&1) {
                        Some((be32(m, 20)?, be64(m, 24)?))
                    } else {
                        Some((be32(m, 12)?, be32(m, 16)? as u64))
                    }
                });
                if let Some((ts, d)) = mdur {
                    if ts > 0 && d > 0 {
                        info.fps = frames as f64 / (d as f64 / ts as f64);
                    }
                }
                info.video = Some(Video {
                    width: w,
                    height: h,
                    codec,
                    frames,
                });
            }
            b"soun" => info.audio = Some(codec),
            _ => {}
        }
    }
    Ok(info)
}

fn mp4_finding(bytes: &[u8]) -> (bool, String) {
    match parse_mp4(bytes) {
        Err(e) => (false, e),
        Ok(m) => match &m.video {
            None => (false, "MP4 has no video track".into()),
            Some(v) => {
                let ok = v.width > 0 && v.height > 0 && v.frames > 0 && m.duration > 0.0;
                (
                    ok,
                    format!(
                        "{}x{} {} {:.2}s {:.1} fps {} frames, audio: {}",
                        v.width,
                        v.height,
                        v.codec,
                        m.duration,
                        m.fps,
                        v.frames,
                        m.audio.as_deref().unwrap_or("none")
                    ),
                )
            }
        },
    }
}

// ------------------------------------------------------------------ platforms

pub struct Platform {
    pub id: &'static str,
    pub name: &'static str,
    pub max_secs: f64,
    pub min_secs: f64,
    pub max_bytes: u64,
    pub min_side: u32,
    pub max_side: u32,
    /// Square or vertical only (YouTube Shorts).
    pub vertical_only: bool,
    pub fps: (f64, f64),
    pub codecs: &'static [&'static str],
}

const GB: u64 = 1 << 30;

/// Limits for publishing through each platform's official API, checked 2026-11-06
/// (https://post0.ai/guides/tiktok-reels-shorts-video-specs, which cites the platform docs).
/// Platforms change these; the skill tells agents to confirm against the platform's own docs.
pub const PLATFORMS: &[Platform] = &[
    Platform {
        id: "tiktok",
        name: "TikTok",
        max_secs: 180.0,
        min_secs: 1.0,
        max_bytes: 4 * GB,
        min_side: 360,
        max_side: 4096,
        vertical_only: false,
        fps: (23.0, 60.0),
        codecs: &["avc1", "hvc1", "hev1", "vp09", "av01"],
    },
    Platform {
        id: "reels",
        name: "Instagram Reels",
        max_secs: 900.0,
        min_secs: 3.0,
        max_bytes: GB,
        min_side: 1,
        max_side: 1920,
        vertical_only: false,
        fps: (23.0, 60.0),
        codecs: &["avc1", "hvc1", "hev1"],
    },
    Platform {
        id: "shorts",
        name: "YouTube Shorts",
        max_secs: 180.0,
        min_secs: 1.0,
        max_bytes: 256 * GB,
        min_side: 1,
        max_side: 8192,
        vertical_only: true,
        fps: (24.0, 60.0),
        codecs: &["avc1", "hvc1", "hev1", "vp09", "av01"],
    },
];

pub fn platform(id: &str) -> Option<&'static Platform> {
    PLATFORMS.iter().find(|p| p.id == id)
}

/// Check one parsed MP4 against one platform. Each entry is (ok, message); a failing entry
/// means the platform would refuse or mis-handle the file.
pub fn check_platform(m: &Mp4, p: &Platform) -> Vec<(bool, String)> {
    let mut r = Vec::new();
    let Some(v) = &m.video else {
        return vec![(false, "no video track".into())];
    };
    r.push((
        m.duration >= p.min_secs && m.duration <= p.max_secs,
        format!(
            "length {:.1}s (allowed {}s to {}s)",
            m.duration, p.min_secs, p.max_secs
        ),
    ));
    r.push((
        m.bytes <= p.max_bytes,
        format!(
            "size {:.1} MiB (limit {} GiB)",
            m.bytes as f64 / 1048576.0,
            p.max_bytes >> 30
        ),
    ));
    let (lo, hi) = (v.width.min(v.height), v.width.max(v.height));
    r.push((
        lo >= p.min_side && hi <= p.max_side,
        format!(
            "{}x{} (each side {} to {} px)",
            v.width, v.height, p.min_side, p.max_side
        ),
    ));
    if p.vertical_only {
        r.push((
            v.width <= v.height,
            "square or vertical frame (wider than square is not a Short)".into(),
        ));
    }
    r.push((
        m.fps >= p.fps.0 - 0.5 && m.fps <= p.fps.1 + 0.5,
        format!("{:.1} fps (allowed {} to {})", m.fps, p.fps.0, p.fps.1),
    ));
    r.push((
        p.codecs.contains(&v.codec.as_str()),
        format!("video codec {}", v.codec),
    ));
    r.push((
        m.audio.is_some(),
        match &m.audio {
            Some(a) => format!("audio track {a}"),
            None => {
                "no audio track (most platforms play silent video, but sound drives reach)".into()
            }
        },
    ));
    r
}

// ------------------------------------------------------------------ files

pub fn file(path: &Path) -> Option<Finding> {
    let bytes = fs::read(path).ok()?;
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    let (ok, detail) = match ext.as_str() {
        "mp4" | "mov" => mp4_finding(&bytes),
        "png" => png(&bytes),
        "json" => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(_) => (true, "valid JSON".into()),
            Err(e) => (false, format!("invalid JSON: {e}")),
        },
        _ => return None,
    };
    Some(Finding {
        file: format!(
            "{} ({} B)",
            path.file_name()?.to_string_lossy(),
            bytes.len()
        ),
        ok,
        detail,
    })
}

pub fn dir(path: &Path) -> std::io::Result<Vec<Finding>> {
    let mut names: Vec<_> = fs::read_dir(path)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    names.sort();
    Ok(names.iter().filter_map(|p| file(p)).collect())
}

/// Decode a PNG and report size, distinct colours and mean luma. Blank or flat frames fail.
pub fn png_stats(b: &[u8]) -> Result<(u32, u32, usize, u64), String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(b));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec.read_info().map_err(|e| format!("bad PNG: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("bad PNG data: {e}"))?;
    let ch = info.color_type.samples();
    let px = &buf[..info.buffer_size()];
    let (mut colors, mut lum) = (HashSet::new(), 0u64);
    for p in px.chunks_exact(ch) {
        let (r, g, bl) = if ch >= 3 {
            (p[0], p[1], p[2])
        } else {
            (p[0], p[0], p[0])
        };
        colors.insert((r as u32) << 16 | (g as u32) << 8 | bl as u32);
        lum += (r as u64 + g as u64 + bl as u64) / 3;
    }
    let n = (px.len() / ch).max(1) as u64;
    Ok((info.width, info.height, colors.len(), lum / n))
}

fn png(b: &[u8]) -> (bool, String) {
    match png_stats(b) {
        Err(e) => (false, e),
        Ok((w, h, colors, mean)) => (
            colors > 16 && mean > 1 && mean < 254,
            format!("{w}x{h} PNG, {colors} distinct colors, mean luma {mean}/255"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(ty: &[u8; 4], content: &[u8]) -> Vec<u8> {
        let mut v = ((content.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(ty);
        v.extend_from_slice(content);
        v
    }

    fn mp4(w: u32, h: u32, secs: u32, fps: u32, with_audio: bool) -> Vec<u8> {
        let mut mvhd = vec![0u8; 100];
        mvhd[12..16].copy_from_slice(&1000u32.to_be_bytes());
        mvhd[16..20].copy_from_slice(&(secs * 1000).to_be_bytes());
        let mut tkhd = vec![0u8; 84];
        tkhd[76..80].copy_from_slice(&(w << 16).to_be_bytes());
        tkhd[80..84].copy_from_slice(&(h << 16).to_be_bytes());
        let mut hdlr = vec![0u8; 8];
        hdlr.extend_from_slice(b"vide");
        let mut mdhd = vec![0u8; 24];
        mdhd[12..16].copy_from_slice(&600u32.to_be_bytes());
        mdhd[16..20].copy_from_slice(&(secs * 600).to_be_bytes());
        let mut stsd = vec![0u8; 8];
        stsd.extend_from_slice(&[0, 0, 0, 16]);
        stsd.extend_from_slice(b"avc1");
        let mut stsz = vec![0u8; 8];
        stsz.extend_from_slice(&(secs * fps).to_be_bytes());
        let stbl = [bx(b"stsd", &stsd), bx(b"stsz", &stsz)].concat();
        let minf = bx(b"stbl", &stbl);
        let mdia = [bx(b"mdhd", &mdhd), bx(b"hdlr", &hdlr), bx(b"minf", &minf)].concat();
        let mut moov = [
            bx(b"mvhd", &mvhd),
            bx(b"trak", &[bx(b"tkhd", &tkhd), bx(b"mdia", &mdia)].concat()),
        ]
        .concat();
        if with_audio {
            let mut ah = vec![0u8; 8];
            ah.extend_from_slice(b"soun");
            let mut asd = vec![0u8; 8];
            asd.extend_from_slice(&[0, 0, 0, 16]);
            asd.extend_from_slice(b"mp4a");
            let am = bx(b"minf", &bx(b"stbl", &bx(b"stsd", &asd)));
            let amdia = [bx(b"hdlr", &ah), am].concat();
            moov.extend(bx(b"trak", &bx(b"mdia", &amdia)));
        }
        [bx(b"ftyp", b"isom0000"), bx(b"moov", &moov)].concat()
    }

    #[test]
    fn parses_dimensions_duration_fps_codec_and_audio() {
        let m = parse_mp4(&mp4(1080, 1920, 10, 30, true)).unwrap();
        let v = m.video.unwrap();
        assert_eq!(
            (v.width, v.height, v.codec.as_str(), v.frames),
            (1080, 1920, "avc1", 300)
        );
        assert!((m.duration - 10.0).abs() < 1e-9 && (m.fps - 30.0).abs() < 1e-9);
        assert_eq!(m.audio.as_deref(), Some("mp4a"));
    }

    #[test]
    fn rejects_non_mp4_and_unfinalized_files() {
        assert!(parse_mp4(b"not a movie at all").is_err());
        assert!(parse_mp4(&bx(b"ftyp", b"isom0000"))
            .unwrap_err()
            .contains("moov"));
        let (ok, _) = mp4_finding(&mp4(0, 0, 1, 30, false));
        assert!(!ok);
    }

    #[test]
    fn platform_limits_flag_the_right_problems() {
        let m = parse_mp4(&mp4(1080, 1920, 10, 30, true)).unwrap();
        for p in PLATFORMS {
            assert!(check_platform(&m, p).iter().all(|(ok, _)| *ok), "{}", p.id);
        }
        let wide = parse_mp4(&mp4(1920, 1080, 10, 30, true)).unwrap();
        let shorts = check_platform(&wide, platform("shorts").unwrap());
        assert!(shorts.iter().any(|(ok, msg)| !ok && msg.contains("Short")));
        let long = parse_mp4(&mp4(1080, 1920, 200, 30, true)).unwrap();
        assert!(check_platform(&long, platform("tiktok").unwrap())
            .iter()
            .any(|(ok, msg)| !ok && msg.starts_with("length")));
        let reel_short = parse_mp4(&mp4(1080, 1920, 2, 30, true)).unwrap();
        assert!(check_platform(&reel_short, platform("reels").unwrap())
            .iter()
            .any(|(ok, msg)| !ok && msg.starts_with("length")));
    }

    #[test]
    fn png_check_rejects_flat_frames() {
        let mut flat = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut flat, 32, 32);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&vec![0u8; 32 * 32 * 3]).unwrap();
        }
        assert!(!png(&flat).0);
        assert!(!png(b"nope").0);
    }
}
