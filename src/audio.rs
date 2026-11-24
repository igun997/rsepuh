//! Small audio helpers, std only: WAV read/write, silence trimming and MP3 duration, so voice
//! clips can be measured exactly and the timeline never drifts from the sound.

pub fn wav_from_pcm16(pcm: &[u8], rate: u32, channels: u16) -> Vec<u8> {
    let mut v = Vec::with_capacity(44 + pcm.len());
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&channels.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    v.extend_from_slice(&(channels * 2).to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    v.extend_from_slice(pcm);
    v
}

/// Seconds of speech to keep before and after the detected speech when trimming.
const PAD_BEFORE: f64 = 0.04;
const PAD_AFTER: f64 = 0.08;

/// Byte range of 16-bit mono PCM to keep: from the first to the last window louder than
/// `thresh_db` dBFS, with a little padding. `None` when the clip is silent.
pub fn speech_range(pcm: &[u8], rate: u32, thresh_db: f64) -> Option<(usize, usize)> {
    let samples = pcm.len() / 2;
    let win = (rate as usize / 100).max(1); // 10 ms
    let loud = |w: usize| -> bool {
        let start = w * win;
        let end = (start + win).min(samples);
        if start >= end {
            return false;
        }
        let mut acc = 0f64;
        for i in start..end {
            let s = i16::from_le_bytes([pcm[i * 2], pcm[i * 2 + 1]]) as f64 / 32768.0;
            acc += s * s;
        }
        let rms = (acc / (end - start) as f64).sqrt();
        rms > 0.0 && 20.0 * rms.log10() > thresh_db
    };
    let windows = samples.div_ceil(win);
    let first = (0..windows).find(|&w| loud(w))?;
    let last = (0..windows).rev().find(|&w| loud(w))?;
    let a = (first * win).saturating_sub((PAD_BEFORE * rate as f64) as usize);
    let b = (((last + 1) * win) + (PAD_AFTER * rate as f64) as usize).min(samples);
    Some((a * 2, b * 2))
}

/// Scale 16-bit mono PCM so the RMS of its speech (windows louder than -45 dBFS) is `target_db`,
/// never letting a sample go above `peak_db`. Gives every voice take the same level, so one scene
/// is not louder than the next. Returns the scaled bytes and the gain applied in dB.
pub fn normalize_active(pcm: &[u8], rate: u32, target_db: f64, peak_db: f64) -> (Vec<u8>, f64) {
    let samples: Vec<f64> = pcm
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f64 / 32768.0)
        .collect();
    let win = (rate as usize / 100).max(1);
    let (mut acc, mut n) = (0f64, 0usize);
    for w in samples.chunks(win) {
        let e: f64 = w.iter().map(|v| v * v).sum::<f64>() / w.len() as f64;
        if e > 0.0 && 10.0 * e.log10() > -45.0 {
            acc += w.iter().map(|v| v * v).sum::<f64>();
            n += w.len();
        }
    }
    if n == 0 {
        return (pcm.to_vec(), 0.0);
    }
    let rms_db = 10.0 * (acc / n as f64).log10();
    let peak = samples.iter().fold(0f64, |a, v| a.max(v.abs())).max(1e-9);
    let by_target = target_db - rms_db;
    let by_peak = peak_db - 20.0 * peak.log10();
    let gain_db = by_target.min(by_peak);
    let g = 10f64.powf(gain_db / 20.0);
    let out = samples
        .iter()
        .flat_map(|v| ((v * g * 32767.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes())
        .collect();
    (out, gain_db)
}

/// Silent gaps inside 16-bit mono PCM: (start, end) seconds of every run of 10 ms windows that
/// stays below `thresh_db` for at least `min_len` seconds. Used to anchor word times on the real
/// pauses of a synthesised sentence.
pub fn find_pauses(pcm: &[u8], rate: u32, thresh_db: f64, min_len: f64) -> Vec<(f64, f64)> {
    let samples = pcm.len() / 2;
    let win = (rate as usize / 100).max(1);
    let windows = samples / win;
    let quiet = |w: usize| -> bool {
        let mut acc = 0f64;
        for i in w * win..(w + 1) * win {
            let v = i16::from_le_bytes([pcm[i * 2], pcm[i * 2 + 1]]) as f64 / 32768.0;
            acc += v * v;
        }
        let rms = (acc / win as f64).sqrt();
        rms <= 0.0 || 20.0 * rms.log10() < thresh_db
    };
    let (mut out, mut run): (Vec<(f64, f64)>, Option<usize>) = (Vec::new(), None);
    let secs = |w: usize| w as f64 * win as f64 / rate as f64;
    for w in 0..=windows {
        let q = w < windows && quiet(w);
        match (q, run) {
            (true, None) => run = Some(w),
            (false, Some(a)) => {
                if secs(w) - secs(a) >= min_len {
                    out.push((secs(a), secs(w)));
                }
                run = None;
            }
            _ => {}
        }
    }
    out
}

/// Seconds of audio in an MP3 (Layer III), summing real frame lengths so VBR is exact.
pub fn mp3_duration(b: &[u8]) -> Option<f64> {
    const BR1: [u32; 15] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    const BR2: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    let mut at = 0usize;
    if b.len() > 10 && &b[..3] == b"ID3" {
        let size = b[6..10]
            .iter()
            .fold(0usize, |a, &x| (a << 7) | (x & 0x7f) as usize);
        at = 10 + size;
    }
    let (mut secs, mut frames) = (0f64, 0u32);
    while at + 4 <= b.len() {
        if b[at] != 0xFF || b[at + 1] & 0xE0 != 0xE0 {
            at += 1;
            continue;
        }
        let ver = (b[at + 1] >> 3) & 3; // 3 = MPEG1, 2 = MPEG2, 0 = MPEG2.5
        let layer = (b[at + 1] >> 1) & 3; // 1 = Layer III
        let br_idx = (b[at + 2] >> 4) as usize;
        let sr_idx = ((b[at + 2] >> 2) & 3) as usize;
        let pad = ((b[at + 2] >> 1) & 1) as u32;
        if ver == 1 || layer != 1 || br_idx == 0 || br_idx >= 15 || sr_idx == 3 {
            at += 1;
            continue;
        }
        let (kbps, rate, spf, coef) = match ver {
            3 => (BR1[br_idx], [44100, 48000, 32000][sr_idx], 1152u32, 144u32),
            2 => (BR2[br_idx], [22050, 24000, 16000][sr_idx], 576, 72),
            _ => (BR2[br_idx], [11025, 12000, 8000][sr_idx], 576, 72),
        };
        let len = (coef * kbps * 1000 / rate + pad) as usize;
        if len < 4 {
            at += 1;
            continue;
        }
        secs += spf as f64 / rate as f64;
        frames += 1;
        at += len;
    }
    (frames > 0).then_some(secs)
}

/// (sample rate, channels, data bytes) of a 16-bit PCM WAV.
pub fn parse_wav(b: &[u8]) -> Option<(u32, u16, &[u8])> {
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let (mut at, mut fmt) = (12usize, None);
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into().ok()?) as usize;
        let body = b.get(at + 8..at + 8 + size.min(b.len() - at - 8))?;
        if id == b"fmt " && body.len() >= 16 {
            let ch = u16::from_le_bytes(body[2..4].try_into().ok()?);
            let rate = u32::from_le_bytes(body[4..8].try_into().ok()?);
            let bits = u16::from_le_bytes(body[14..16].try_into().ok()?);
            if bits != 16 {
                return None;
            }
            fmt = Some((rate, ch));
        } else if id == b"data" {
            let (rate, ch) = fmt?;
            return Some((rate, ch, body));
        }
        at += 8 + size + (size & 1);
    }
    None
}

pub fn pcm_seconds(bytes: usize, rate: u32, channels: u16) -> f64 {
    bytes as f64 / (rate as f64 * channels as f64 * 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, secs: f64, amp: f64) -> Vec<u8> {
        (0..(rate as f64 * secs) as usize)
            .flat_map(|i| {
                let v = (amp * 32767.0 * (i as f64 * 0.3).sin()) as i16;
                v.to_le_bytes()
            })
            .collect()
    }

    #[test]
    fn wav_roundtrip_and_duration() {
        let pcm = tone(24000, 1.5, 0.3);
        let wav = wav_from_pcm16(&pcm, 24000, 1);
        let (rate, ch, data) = parse_wav(&wav).unwrap();
        assert_eq!((rate, ch, data.len()), (24000, 1, pcm.len()));
        assert!((pcm_seconds(data.len(), rate, ch) - 1.5).abs() < 1e-9);
        assert!(parse_wav(b"RIFFxxxxWAVE").is_none());
    }

    #[test]
    fn trims_leading_and_trailing_silence_but_keeps_speech() {
        let rate = 24000;
        let mut pcm = vec![0u8; (rate as f64 * 0.6) as usize * 2];
        pcm.extend(tone(rate, 1.0, 0.4));
        pcm.extend(vec![0u8; (rate as f64 * 0.9) as usize * 2]);
        let (a, b) = speech_range(&pcm, rate, -45.0).unwrap();
        let kept = pcm_seconds(b - a, rate, 1);
        assert!((1.08..1.2).contains(&kept), "kept {kept}");
        let start = a as f64 / 2.0 / rate as f64;
        assert!((0.5..0.6).contains(&start), "start {start}");
        assert!(speech_range(&vec![0u8; 4800], rate, -45.0).is_none());
    }

    #[test]
    fn normalize_levels_speech_and_respects_the_peak_ceiling() {
        let rate = 24000;
        let mut quiet = vec![0u8; rate as usize]; // 0.5 s of silence does not count as speech
        quiet.extend(tone(rate, 1.0, 0.05));
        let (out, gain) = normalize_active(&quiet, rate, -19.0, -1.5);
        assert!(gain > 10.0, "quiet speech is raised: {gain}");
        let (_, rms_after, _, _) = {
            let samples: Vec<f64> = out
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f64 / 32768.0)
                .collect();
            let loud: Vec<f64> = samples.iter().copied().filter(|v| v.abs() > 1e-6).collect();
            let r = (loud.iter().map(|v| v * v).sum::<f64>() / loud.len() as f64).sqrt();
            (0, 20.0 * r.log10(), 0, 0)
        };
        assert!(
            (rms_after + 19.0).abs() < 1.0,
            "speech RMS lands on the target: {rms_after}"
        );
        // a spiky clip is limited by the peak ceiling before it reaches the target
        let mut spiky = tone(rate, 0.5, 0.02);
        spiky.extend(tone(rate, 0.01, 0.9));
        let (out, gain) = normalize_active(&spiky, rate, -10.0, -1.5);
        let peak = out
            .chunks_exact(2)
            .map(|c| (i16::from_le_bytes([c[0], c[1]]) as f64 / 32768.0).abs())
            .fold(0.0, f64::max);
        assert!(20.0 * peak.log10() <= -1.4, "peak {peak} gain {gain}");
        assert_eq!(normalize_active(&vec![0u8; 400], rate, -19.0, -1.5).1, 0.0);
    }

    #[test]
    fn finds_inner_pauses_but_not_short_dips() {
        let rate = 24000;
        let mut pcm = tone(rate, 0.5, 0.4);
        pcm.extend(vec![0u8; (rate as f64 * 0.30) as usize * 2]); // a real pause
        pcm.extend(tone(rate, 0.4, 0.4));
        pcm.extend(vec![0u8; (rate as f64 * 0.04) as usize * 2]); // a dip too short to count
        pcm.extend(tone(rate, 0.3, 0.4));
        let p = find_pauses(&pcm, rate, -42.0, 0.08);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            (p[0].0 - 0.5).abs() < 0.02 && (p[0].1 - 0.8).abs() < 0.02,
            "{p:?}"
        );
    }

    #[test]
    fn mp3_duration_sums_frames_for_mpeg2_and_skips_id3() {
        // MPEG2 Layer III, 48 kbps, 24 kHz: 144-byte frames of 576 samples (24 ms)
        let mut frame = vec![0u8; 144];
        frame[..4].copy_from_slice(&[0xFF, 0xF2, 0x64, 0xC0]);
        let mut mp3 = b"ID3\x03\x00\x00\x00\x00\x00\x04abcd".to_vec();
        for _ in 0..250 {
            mp3.extend_from_slice(&frame);
        }
        let d = mp3_duration(&mp3).unwrap();
        assert!((d - 6.0).abs() < 1e-9, "{d}");
        assert!(mp3_duration(b"not mp3 data").is_none());
    }
}
