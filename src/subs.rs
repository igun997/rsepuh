//! Words, cues and subtitle files. Timings come from a voice engine (word boundaries or an
//! estimate), from an SRT/VTT file, or from the scene itself; everything ends up as `Cue`s with
//! per-word times so every subtitle style can highlight the word being spoken.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub w: String,
    pub s: f64,
    pub e: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub s: f64,
    pub e: f64,
    pub words: Vec<Word>,
}

impl Cue {
    pub fn text(&self) -> String {
        self.words
            .iter()
            .map(|w| w.w.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
    pub fn to_json(&self) -> Value {
        json!({
            "s": r3(self.s), "e": r3(self.e),
            "words": self.words.iter().map(|w| json!({"w": w.w, "s": r3(w.s), "e": r3(w.e)})).collect::<Vec<_>>()
        })
    }
}

fn r3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn sentence_end(w: &str) -> bool {
    w.trim_end_matches(['"', '\'', ')', '”'])
        .ends_with(['.', '!', '?', '…'])
}

/// How long a token takes relative to others: its letters, plus a pause after punctuation.
fn weight(w: &str) -> (f64, f64) {
    let letters = w.chars().filter(|c| c.is_alphanumeric()).count().max(1) as f64;
    let pause = if sentence_end(w) {
        4.0
    } else if w.ends_with([',', ';', ':', '—', '-']) {
        2.0
    } else {
        0.0
    };
    (letters, pause)
}

/// Spread whitespace-separated tokens over `[s, e]` in proportion to their length, leaving a
/// pause after commas and full stops. The estimate used when no engine reports word times.
pub fn distribute(text: &str, s: f64, e: f64) -> Vec<Word> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() || e <= s {
        return Vec::new();
    }
    let ws: Vec<(f64, f64)> = tokens.iter().map(|t| weight(t)).collect();
    // no pause after the last word: the clip ends with it
    let total: f64 = ws.iter().map(|(l, _)| l).sum::<f64>()
        + ws[..ws.len() - 1].iter().map(|(_, p)| p).sum::<f64>();
    let span = e - s;
    let mut cur = s;
    tokens
        .iter()
        .zip(&ws)
        .enumerate()
        .map(|(i, (t, (l, p)))| {
            let start = cur;
            let end = start + l / total * span;
            cur = end
                + if i + 1 < tokens.len() {
                    p / total * span
                } else {
                    0.0
                };
            Word {
                w: (*t).to_string(),
                s: start,
                e: end,
            }
        })
        .collect()
}

/// Like [`distribute`], but anchored on measured pauses. `span` is the speech (start, end);
/// `pauses` are the silent gaps inside it. Pauses are matched to the script's punctuation (full
/// stops before commas, in reading order), and each stretch of words is spread over the speech
/// between two pauses. Fewer or more pauses than punctuation never breaks it: unmatched
/// punctuation stays a weighted pause inside its stretch.
pub fn anchored(text: &str, span: (f64, f64), pauses: &[(f64, f64)]) -> Vec<Word> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.len() < 2 || pauses.is_empty() {
        return distribute(text, span.0, span.1);
    }
    // boundaries after token i (not the last) that end in punctuation, strongest first
    let mut bounds: Vec<(usize, f64)> = tokens[..tokens.len() - 1]
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let (_, p) = weight(t);
            (p > 0.0).then_some((i, p))
        })
        .collect();
    if bounds.is_empty() {
        return distribute(text, span.0, span.1);
    }
    let m = bounds.len().min(pauses.len());
    // keep the m strongest boundaries (earlier wins a tie), and the m longest pauses
    bounds.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    bounds.truncate(m);
    bounds.sort_by_key(|b| b.0);
    let mut ps: Vec<(f64, f64)> = pauses.to_vec();
    ps.sort_by(|a, b| (b.1 - b.0).total_cmp(&(a.1 - a.0)));
    ps.truncate(m);
    ps.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut out = Vec::new();
    let (mut from_tok, mut from_t) = (0usize, span.0);
    for (k, (bi, _)) in bounds.iter().enumerate() {
        let group = tokens[from_tok..=*bi].join(" ");
        out.extend(distribute(&group, from_t, ps[k].0.max(from_t + 0.02)));
        from_tok = bi + 1;
        from_t = ps[k].1;
    }
    let group = tokens[from_tok..].join(" ");
    out.extend(distribute(&group, from_t.min(span.1 - 0.02), span.1));
    out
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Give the script's own tokens (with their punctuation and capitals) the times an engine
/// reported for its spoken tokens. `spoken` is (text, start, end). Falls back to a proportional
/// spread over the spoken span when the two token streams do not line up.
pub fn align(script: &str, spoken: &[(String, f64, f64)]) -> Vec<Word> {
    let tokens: Vec<&str> = script.split_whitespace().collect();
    if tokens.is_empty() || spoken.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut j = 0usize;
    let mut ok = true;
    for t in &tokens {
        let want = norm(t);
        if want.is_empty() {
            // pure punctuation ("-", "&"): attach to the previous word's time
            if let Some(prev) = out.last().cloned() {
                let prev: Word = prev;
                out.push(Word {
                    w: (*t).to_string(),
                    s: prev.e,
                    e: prev.e,
                });
            }
            continue;
        }
        let (mut got, start, mut end) = (String::new(), spoken.get(j).map(|x| x.1), 0.0);
        while j < spoken.len() && got.len() < want.len() {
            got += &norm(&spoken[j].0);
            end = spoken[j].2;
            j += 1;
        }
        match start {
            Some(s) if got == want => out.push(Word {
                w: (*t).to_string(),
                s,
                e: end,
            }),
            _ => {
                ok = false;
                break;
            }
        }
    }
    if ok && j == spoken.len() {
        return out;
    }
    distribute(script, spoken[0].1, spoken[spoken.len() - 1].2)
}

/// Group words into on-screen cues: at most `max_words` and `max_chars`, a break after a
/// sentence end, and a break across a long silence.
pub fn chunk(words: &[Word], max_words: usize, max_chars: usize) -> Vec<Cue> {
    let mut cues: Vec<Cue> = Vec::new();
    let mut cur: Vec<Word> = Vec::new();
    let mut chars = 0usize;
    let flush = |cur: &mut Vec<Word>, cues: &mut Vec<Cue>| {
        if let (Some(a), Some(b)) = (cur.first(), cur.last()) {
            cues.push(Cue {
                s: a.s,
                e: b.e,
                words: std::mem::take(cur),
            });
        }
    };
    for w in words {
        let gap = cur.last().map_or(0.0, |p| w.s - p.e);
        let len = w.w.chars().count();
        if !cur.is_empty()
            && (cur.len() >= max_words
                || chars + 1 + len > max_chars
                || gap > 0.5
                || sentence_end(&cur.last().unwrap().w))
        {
            flush(&mut cur, &mut cues);
            chars = 0;
        }
        chars += len + usize::from(!cur.is_empty());
        cur.push(w.clone());
    }
    flush(&mut cur, &mut cues);
    // an orphan (a single word left by a pause) joins the cue after it when they are close
    let mut i = 0;
    while i + 1 < cues.len() {
        let (a, b) = (&cues[i], &cues[i + 1]);
        if max_words > 1
            && a.words.len() == 1
            && b.s - a.e < 0.8
            && a.words.len() + b.words.len() <= max_words + 1
            && a.text().chars().count() + 1 + b.text().chars().count() <= max_chars
            && !sentence_end(&a.words[0].w)
        {
            let mut next = cues.remove(i + 1);
            let mut merged = std::mem::take(&mut cues[i].words);
            merged.append(&mut next.words);
            cues[i] = Cue {
                s: cues[i].s,
                e: next.e,
                words: merged,
            };
        } else {
            i += 1;
        }
    }
    // hold each cue a little past its last word, but never into the next cue
    for i in 0..cues.len() {
        let limit = cues.get(i + 1).map_or(f64::MAX, |n| n.s);
        cues[i].e = (cues[i].e + 0.12).min(limit).max(cues[i].e);
    }
    cues
}

fn clock(t: &str) -> Option<f64> {
    let t = t.trim().replace(',', ".");
    let parts: Vec<&str> = t.split(':').collect();
    let (h, m, s) = match parts.as_slice() {
        [h, m, s] => (
            h.parse::<f64>().ok()?,
            m.parse::<f64>().ok()?,
            s.parse::<f64>().ok()?,
        ),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    Some(h * 3600.0 + m * 60.0 + s)
}

/// Parse SRT or WebVTT into (start, end, text). Tags like `<b>` are removed.
pub fn parse_cues(text: &str) -> Vec<(f64, f64, String)> {
    let text = text.replace("\r\n", "\n");
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(i) = lines.iter().position(|l| l.contains("-->")) else {
            continue;
        };
        let Some((a, b)) = lines[i].split_once("-->") else {
            continue;
        };
        let b = b.split_whitespace().next().unwrap_or("");
        let (Some(s), Some(e)) = (clock(a), clock(b)) else {
            continue;
        };
        let mut body = lines[i + 1..].join(" ");
        while let (Some(l), Some(r)) = (body.find('<'), body.find('>')) {
            if r < l {
                break;
            }
            body.replace_range(l..=r, "");
        }
        let body = body.split_whitespace().collect::<Vec<_>>().join(" ");
        if !body.is_empty() && e > s {
            out.push((s, e, body));
        }
    }
    out
}

fn stamp(t: f64, sep: char) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}{sep}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

pub fn to_srt(cues: &[Cue]) -> String {
    cues.iter()
        .enumerate()
        .map(|(i, c)| {
            format!(
                "{}\n{} --> {}\n{}\n",
                i + 1,
                stamp(c.s, ','),
                stamp(c.e, ','),
                c.text()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn to_vtt(cues: &[Cue]) -> String {
    let body: Vec<String> = cues
        .iter()
        .map(|c| {
            format!(
                "{} --> {}\n{}\n",
                stamp(c.s, '.'),
                stamp(c.e, '.'),
                c.text()
            )
        })
        .collect();
    format!("WEBVTT\n\n{}", body.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(t: &str, s: f64, e: f64) -> Word {
        Word { w: t.into(), s, e }
    }

    #[test]
    fn distribute_covers_the_span_in_order_with_pauses_after_punctuation() {
        let ws = distribute("Halo, dunia ini bagus.", 1.0, 3.0);
        assert_eq!(ws.len(), 4);
        assert!((ws[0].s - 1.0).abs() < 1e-9 && (ws[3].e - 3.0).abs() < 1e-9);
        for p in ws.windows(2) {
            assert!(p[1].s >= p[0].e - 1e-9);
        }
        assert!(ws[1].s - ws[0].e > 0.1, "pause after the comma");
        assert!(ws[2].s - ws[1].e < 1e-9, "no pause without punctuation");
        assert!(distribute("", 0.0, 1.0).is_empty() && distribute("a", 2.0, 1.0).is_empty());
    }

    #[test]
    fn anchored_places_words_on_each_side_of_a_real_pause() {
        // "one two. three four" with a measured pause 1.0..1.4 inside speech 0..2.2
        let ws = anchored("one two. three four", (0.0, 2.2), &[(1.0, 1.4)]);
        assert_eq!(ws.len(), 4);
        assert!(
            ws[1].e <= 1.0 + 1e-9,
            "'two.' ends at the pause start: {:?}",
            ws[1]
        );
        assert!(
            (ws[2].s - 1.4).abs() < 1e-9,
            "'three' starts after the pause: {:?}",
            ws[2]
        );
        assert!((ws[3].e - 2.2).abs() < 1e-9);
        // more pauses than punctuation: only the strongest boundary is anchored
        let many = anchored(
            "a b. c d",
            (0.0, 3.0),
            &[(0.5, 0.6), (1.2, 1.6), (2.2, 2.3)],
        );
        assert!(
            (many[2].s - 1.6).abs() < 1e-9,
            "longest pause is used: {many:?}"
        );
        // fewer pauses than punctuation: the full stop wins over the comma
        let few = anchored("a, b. c", (0.0, 3.0), &[(1.5, 1.9)]);
        assert!((few[2].s - 1.9).abs() < 1e-9, "{few:?}");
        // no pauses or no punctuation: plain proportional spread
        assert_eq!(
            anchored("a b c", (0.0, 1.0), &[(0.4, 0.5)]),
            distribute("a b c", 0.0, 1.0)
        );
        assert_eq!(
            anchored("a. b", (0.0, 1.0), &[]),
            distribute("a. b", 0.0, 1.0)
        );
    }

    #[test]
    fn align_keeps_script_punctuation_and_engine_times() {
        let spoken: Vec<(String, f64, f64)> = vec![
            ("Halo".into(), 0.1, 0.4),
            ("semuanya".into(), 0.45, 1.2),
            ("Ini".into(), 2.0, 2.2),
        ];
        let ws = align("Halo, semuanya. Ini", &spoken);
        assert_eq!(ws[0], w("Halo,", 0.1, 0.4));
        assert_eq!(ws[1], w("semuanya.", 0.45, 1.2));
        assert_eq!(ws[2], w("Ini", 2.0, 2.2));
    }

    #[test]
    fn align_merges_engine_tokens_and_falls_back_when_streams_differ() {
        // the engine split "10s" into two spoken tokens
        let spoken: Vec<(String, f64, f64)> = vec![
            ("10".into(), 0.0, 0.3),
            ("s".into(), 0.3, 0.5),
            ("ok".into(), 0.6, 0.9),
        ];
        let ws = align("10s ok", &spoken);
        assert_eq!((ws[0].s, ws[0].e, ws[1].s), (0.0, 0.5, 0.6));
        // completely different text: proportional spread over the spoken span
        let ws = align("totally different words here", &spoken);
        assert_eq!(ws.len(), 4);
        assert!((ws[0].s - 0.0).abs() < 1e-9 && (ws[3].e - 0.9).abs() < 1e-9);
    }

    #[test]
    fn chunk_breaks_on_size_sentence_and_silence() {
        let ws = vec![
            w("one", 0.0, 0.3),
            w("two", 0.3, 0.6),
            w("three.", 0.6, 1.0),
            w("four", 1.0, 1.3),
            w("five", 2.5, 2.8),
        ];
        let cues = chunk(&ws, 5, 40);
        let texts: Vec<String> = cues.iter().map(|c| c.text()).collect();
        assert_eq!(texts, vec!["one two three.", "four", "five"]);
        let small = chunk(&ws[..2], 1, 40);
        assert_eq!(small.len(), 2);
        let wide = chunk(&[w("abcdefgh", 0.0, 0.5), w("ijklmnop", 0.5, 1.0)], 9, 10);
        assert_eq!(wide.len(), 2, "max_chars splits");
        assert!(cues[0].e >= 1.0 && cues[0].e <= cues[1].s);
    }

    #[test]
    fn orphan_words_join_the_next_cue_when_close() {
        let ws = vec![
            w("That", 0.0, 0.2),
            w("model", 0.2, 0.5),
            w("gets", 0.5, 0.7),
            w("you", 0.7, 0.9),
            w("far,", 0.9, 1.2),
            w("until", 1.7, 1.9),
            w("it", 1.9, 2.0),
            w("breaks.", 2.0, 2.5),
        ];
        let cues = chunk(&ws, 4, 40);
        let texts: Vec<String> = cues.iter().map(|c| c.text()).collect();
        assert_eq!(texts, vec!["That model gets you", "far, until it breaks."]);
        // far apart: stays alone
        let far = chunk(
            &[w("a", 0.0, 0.3), w("b", 2.0, 2.3), w("c", 2.3, 2.6)],
            4,
            40,
        );
        assert_eq!(far.len(), 2);
    }

    #[test]
    fn srt_and_vtt_parse_and_write_round_trip() {
        let src = "1\n00:00:01,000 --> 00:00:02,500\n<i>Halo</i> dunia\n\n2\n00:00:03,000 --> 00:00:04,000\nlagi\n";
        let cues = parse_cues(src);
        assert_eq!(
            cues,
            vec![(1.0, 2.5, "Halo dunia".into()), (3.0, 4.0, "lagi".into())]
        );
        let vtt = "WEBVTT\n\n00:01.000 --> 00:02.000 align:center\nsatu\n";
        assert_eq!(parse_cues(vtt), vec![(1.0, 2.0, "satu".into())]);
        let out = to_srt(&[Cue {
            s: 1.0,
            e: 2.5,
            words: distribute("Halo dunia", 1.0, 2.5),
        }]);
        assert_eq!(out, "1\n00:00:01,000 --> 00:00:02,500\nHalo dunia\n");
        assert!(to_vtt(&[Cue {
            s: 61.5,
            e: 62.0,
            words: vec![w("x", 61.5, 62.0)]
        }])
        .contains("00:01:01.500 --> 00:01:02.000"));
    }
}
