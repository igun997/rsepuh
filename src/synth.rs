//! Handcrafted music and sound effects, std only. The bed is built on an exact beat grid
//! (`bpm`, starting at t = 0), so scene cuts, transitions and pops that sit on that grid land on
//! the kick. Sound effects are placed from the same events the picture uses.
//!
//! Everything is deterministic: the same inputs always produce the same samples.

use std::f64::consts::PI;

pub const RATE: u32 = 44_100;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        // xorshift64*, mapped to -1..1
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let v = self.0.wrapping_mul(0x2545F4914F6CDD1D);
        (v >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

fn midi_hz(note: f64) -> f64 {
    440.0 * 2f64.powf((note - 69.0) / 12.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Four on the floor, offbeat bass, pumping chords: the default for hype and product videos.
    House,
    /// Slower, swung, soft: calm explainers and lifestyle.
    Lofi,
}

impl Style {
    pub fn parse(s: &str) -> Option<Style> {
        match s {
            "house" => Some(Style::House),
            "lofi" => Some(Style::Lofi),
            _ => None,
        }
    }
}

/// One mono buffer with additive mixing at a time position.
struct Track {
    s: Vec<f64>,
}

impl Track {
    fn new(secs: f64) -> Track {
        Track {
            s: vec![0.0; (secs * RATE as f64).ceil() as usize + 1],
        }
    }
    fn add(&mut self, at: f64, gain: f64, voice: &[f64]) {
        let start = (at * RATE as f64).round() as isize;
        for (i, v) in voice.iter().enumerate() {
            let k = start + i as isize;
            if k >= 0 && (k as usize) < self.s.len() {
                self.s[k as usize] += v * gain;
            }
        }
    }
    fn to_pcm(&self, peak_db: f64) -> Vec<u8> {
        let max = self.s.iter().fold(0.0f64, |a, b| a.max(b.abs())).max(1e-9);
        let g = 10f64.powf(peak_db / 20.0) / max;
        self.s
            .iter()
            .flat_map(|v| ((v * g * 32767.0).round() as i16).to_le_bytes())
            .collect()
    }
}

fn env_exp(t: f64, decay: f64) -> f64 {
    (-t / decay).exp()
}

fn kick() -> Vec<f64> {
    let n = (0.38 * RATE as f64) as usize;
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let f = 46.0 + 120.0 * (-t / 0.035).exp();
            phase += 2.0 * PI * f / RATE as f64;
            let click = if t < 0.004 {
                0.6 * (1.0 - t / 0.004)
            } else {
                0.0
            };
            (phase.sin() * env_exp(t, 0.16) + click) * 1.0
        })
        .collect()
}

/// Noise burst with a one-pole highpass (cheap "tss"/clap body) and an exponential decay.
fn noise_hit(rng: &mut Rng, dur: f64, decay: f64, hp: f64, taps: &[f64]) -> Vec<f64> {
    let n = (dur * RATE as f64) as usize;
    let a = (-2.0 * PI * hp / RATE as f64).exp();
    let (mut prev_in, mut prev_out) = (0.0, 0.0);
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let x = rng.next();
            let y = a * (prev_out + x - prev_in);
            prev_in = x;
            prev_out = y;
            // claps are a few fast re-triggers; hats pass a single tap at 0
            let e: f64 = taps
                .iter()
                .map(|&d| if t >= d { env_exp(t - d, decay) } else { 0.0 })
                .sum();
            y * e.min(1.5)
        })
        .collect()
}

/// Band-limited-ish saw by summing a few harmonics, then a gentle lowpass via averaging.
fn saw_note(freq: f64, dur: f64, decay: f64, harmonics: usize) -> Vec<f64> {
    let n = (dur * RATE as f64) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let mut v = 0.0;
            for h in 1..=harmonics {
                let f = freq * h as f64;
                if f > 9000.0 {
                    break;
                }
                v += (2.0 * PI * f * t).sin() / h as f64;
            }
            let attack = (t / 0.004).min(1.0);
            v * env_exp(t, decay) * attack * 0.5
        })
        .collect()
}

fn sub_note(freq: f64, dur: f64) -> Vec<f64> {
    let n = (dur * RATE as f64) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            let attack = (t / 0.005).min(1.0);
            let release = ((dur - t) / 0.03).clamp(0.0, 1.0);
            ((2.0 * PI * freq * t).sin() + 0.25 * (2.0 * PI * 2.0 * freq * t).sin())
                * attack
                * release
        })
        .collect()
}

/// Chord roots (MIDI) of a minor four-chord loop: Am F C G, and each chord's three notes.
const PROG: [(f64, [f64; 3]); 4] = [
    (33.0, [57.0, 60.0, 64.0]), // Am: A1 bass, A3 C4 E4
    (29.0, [53.0, 57.0, 60.0]), // F:  F1 bass, F3 A3 C4
    (36.0, [60.0, 64.0, 67.0]), // C:  C2 bass, C4 E4 G4
    (31.0, [55.0, 59.0, 62.0]), // G:  G1 bass, G3 B3 D4
];

/// A music bed of `secs` seconds at `bpm`, beat 1 exactly at t = 0.
/// Arrangement: bar 1 kick and hats, bar 2 adds the bass, bar 3 on adds claps, chord stabs and an
/// arpeggio; the last bar drops the tonal parts and ends on a crash.
pub fn music(bpm: f64, secs: f64, style: Style) -> Vec<u8> {
    let beat = 60.0 / bpm;
    let mut drums = Track::new(secs);
    let mut tonal = Track::new(secs);
    let mut pump = vec![1.0f64; drums.s.len()]; // sidechain multiplier for the tonal parts
    let mut rng = Rng(0x9E3779B97F4A7C15);
    let kick_v = kick();
    let clap_v = noise_hit(&mut rng, 0.22, 0.05, 1200.0, &[0.0, 0.012, 0.024]);
    let hat_v = noise_hit(&mut rng, 0.07, 0.012, 7000.0, &[0.0]);
    let ohat_v = noise_hit(&mut rng, 0.18, 0.06, 6000.0, &[0.0]);
    let swing = if style == Style::Lofi { 0.12 } else { 0.0 };
    let total_beats = (secs / beat).floor() as usize;

    for b in 0..total_beats {
        let t = b as f64 * beat;
        let (bar, in_bar) = (b / 4, b % 4);
        let last_bar = b + 4 >= total_beats && total_beats >= 8;
        if style == Style::House || in_bar % 2 == 0 {
            drums.add(t, if style == Style::House { 1.0 } else { 0.85 }, &kick_v);
            let start = (t * RATE as f64) as usize;
            for i in 0..(0.3 * RATE as f64) as usize {
                if let Some(p) = pump.get_mut(start + i) {
                    *p = p.min(0.2 + 0.8 * (i as f64 / (0.26 * RATE as f64)).min(1.0));
                }
            }
        }
        if bar >= 2 && (in_bar == 1 || in_bar == 3) {
            drums.add(t, 0.55, &clap_v);
        }
        drums.add(t + beat * (0.5 + swing * 0.5), 0.30, &ohat_v);
        for q in [0.25f64, 0.75] {
            drums.add(t + beat * q, 0.13, &hat_v);
        }
        let (root, chord) = PROG[bar % 4];
        if bar >= 1 && !last_bar {
            tonal.add(t + beat * 0.5, 0.6, &sub_note(midi_hz(root), beat * 0.42));
        }
        if bar >= 2 && !last_bar {
            if in_bar == 1 || in_bar == 3 {
                for n in chord {
                    tonal.add(
                        t + beat * 0.5,
                        0.10,
                        &saw_note(midi_hz(n), beat * 0.9, 0.22, 9),
                    );
                }
            }
            let arp = chord[(b * 2) % 3] + 12.0;
            tonal.add(
                t + beat * 0.25,
                0.05,
                &saw_note(midi_hz(arp), beat * 0.4, 0.1, 6),
            );
        }
    }
    let end = total_beats as f64 * beat;
    if end <= secs + 0.01 {
        let crash = noise_hit(&mut rng, 1.4, 0.45, 3500.0, &[0.0]);
        drums.add(end.min(secs - 0.05).max(0.0), 0.35, &crash);
    }
    let mut out = Track::new(secs);
    for (i, o) in out.s.iter_mut().enumerate() {
        *o = (drums.s[i] + tonal.s[i] * pump[i]).tanh();
    }
    out.to_pcm(-4.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sfx {
    /// Low impact with a noise crack: scene cuts and the end card.
    Impact,
    /// Rising filtered noise that peaks at `at`: placed before a cut.
    Whoosh,
    /// Short pitched blip: stickers, words and buttons landing.
    Pop,
    /// Dry tick: hook words.
    Tick,
}

fn whoosh(rng: &mut Rng, dur: f64) -> Vec<f64> {
    let n = (dur * RATE as f64) as usize;
    let mut lp = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let cutoff = 250.0 + 7500.0 * t * t;
            let a = 1.0 - (-2.0 * PI * cutoff / RATE as f64).exp();
            lp += a * (rng.next() - lp);
            lp * t.powf(1.6) * (1.0 - t).max(0.0).sqrt().max(0.25) * 2.2
        })
        .collect()
}

fn impact(rng: &mut Rng) -> Vec<f64> {
    let n = (0.7 * RATE as f64) as usize;
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            phase += 2.0 * PI * (38.0 + 70.0 * (-t / 0.09).exp()) / RATE as f64;
            let body = phase.sin() * env_exp(t, 0.28);
            let crack = rng.next() * env_exp(t, 0.025) * 0.8;
            body + crack
        })
        .collect()
}

fn pop_blip(freq: f64) -> Vec<f64> {
    let n = (0.09 * RATE as f64) as usize;
    let mut phase = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f64 / RATE as f64;
            phase += 2.0 * PI * freq * (1.0 + 0.9 * (-t / 0.02).exp()) / RATE as f64;
            phase.sin() * env_exp(t, 0.03)
        })
        .collect()
}

/// Sound effects for `events` (time in seconds, kind), `secs` long. A cut gets a whoosh that
/// builds into an impact exactly on the cut.
pub fn sfx(events: &[(f64, Sfx)], secs: f64) -> Vec<u8> {
    let mut tr = Track::new(secs);
    let mut rng = Rng(0xD1B54A32D192ED03);
    let impact_v = impact(&mut rng);
    let tick_v: Vec<f64> = noise_hit(&mut rng, 0.05, 0.01, 2500.0, &[0.0]);
    for (i, (at, kind)) in events.iter().enumerate() {
        match kind {
            Sfx::Impact => tr.add(*at, 0.9, &impact_v),
            Sfx::Whoosh => {
                let w = whoosh(&mut rng, 0.45);
                tr.add(*at - 0.45, 0.45, &w);
            }
            Sfx::Pop => tr.add(*at, 0.5, &pop_blip(740.0 + 110.0 * (i % 3) as f64)),
            Sfx::Tick => tr.add(*at, 0.6, &tick_v),
        }
    }
    tr.to_pcm(-3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(pcm: &[u8]) -> Vec<f64> {
        pcm.chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f64 / 32768.0)
            .collect()
    }

    /// RMS of a 30 ms window starting at `t`.
    fn rms(s: &[f64], t: f64) -> f64 {
        let a = (t * RATE as f64) as usize;
        let w = &s[a.min(s.len())..(a + (0.03 * RATE as f64) as usize).min(s.len())];
        (w.iter().map(|v| v * v).sum::<f64>() / w.len().max(1) as f64).sqrt()
    }

    #[test]
    fn music_is_deterministic_and_the_right_length() {
        let a = music(120.0, 4.0, Style::House);
        assert_eq!(a, music(120.0, 4.0, Style::House));
        let n = a.len() / 2;
        assert!((n as f64 / RATE as f64 - 4.0).abs() < 0.01);
        assert_ne!(a, music(120.0, 4.0, Style::Lofi));
    }

    #[test]
    fn kicks_sit_exactly_on_the_beat_grid() {
        let bpm = 120.0;
        let s = samples(&music(bpm, 6.0, Style::House));
        let beat = 60.0 / bpm;
        for k in 0..10 {
            let on = rms(&s, k as f64 * beat);
            // 80 ms before the kick is quieter than the kick itself
            let before = rms(&s, (k as f64 * beat - 0.08).max(0.0));
            if k > 0 {
                assert!(on > before * 1.3, "beat {k}: on {on} before {before}");
            }
            assert!(on > 0.05, "beat {k} is audible");
        }
        assert!(s.iter().all(|v| v.abs() <= 0.9), "peak is limited");
    }

    #[test]
    fn sfx_place_impacts_on_the_cut_and_whooshes_before_it() {
        let ev = [(1.0, Sfx::Whoosh), (1.0, Sfx::Impact), (2.0, Sfx::Pop)];
        let s = samples(&sfx(&ev, 3.0));
        assert!(rms(&s, 1.0) > 0.1, "impact on the cut");
        assert!(rms(&s, 0.7) > 0.005, "whoosh builds before it");
        assert!(rms(&s, 0.3) < 0.002, "silence before the whoosh");
        assert!(rms(&s, 2.01) > 0.05, "pop lands on time");
        assert!(rms(&s, 2.6) < 0.002, "and ends");
    }

    #[test]
    fn style_names() {
        assert_eq!(Style::parse("house"), Some(Style::House));
        assert_eq!(Style::parse("lofi"), Some(Style::Lofi));
        assert_eq!(Style::parse("trap"), None);
    }
}
