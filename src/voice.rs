//! Voice-over: one clip per scene, measured exactly, with word times for subtitles.
//!
//! Two engines:
//! - `edge` (free): Microsoft Edge read-aloud voices through edge-tts, which reports the time of
//!   every spoken word. Run with `uv run --with edge-tts` so it is always a current release.
//! - `gemini`: Gemini TTS. It returns audio without word times, so each scene is its own request
//!   (scene sync is exact: the clip length is the scene's voice length), silence is trimmed, and
//!   words are spread over the speech in proportion to their length.
//!
//! The result is `assets/voice/<name>/timeline.json`; the spec builder turns it into scene
//! lengths, audio layers and subtitle cues, so picture, sound and words cannot drift apart.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::util::fnv1a;
use crate::{audio, gemini, subs, Result};

pub const GEMINI_TTS_MODEL: &str = "gemini-3.8-flash-tts";
/// Prebuilt Gemini voices (an unknown name is HTTP 400), verified by rveditor on 2026-11-19.
pub const GEMINI_VOICES: [&str; 30] = [
    "Zephyr",
    "Puck",
    "Charon",
    "Kore",
    "Fenrir",
    "Leda",
    "Orus",
    "Aoede",
    "Callirrhoe",
    "Autonoe",
    "Enceladus",
    "Iapetus",
    "Umbriel",
    "Algieba",
    "Despina",
    "Erinome",
    "Algenib",
    "Rasalgethi",
    "Laomedeia",
    "Achernar",
    "Alnilam",
    "Schedar",
    "Gacrux",
    "Pulcherrima",
    "Achird",
    "Zubenelgenubi",
    "Vindemiatrix",
    "Sadachbia",
    "Sadaltager",
    "Sulafat",
];
const MAX_TEXT_CHARS: usize = 1200;
/// RMS of the speech itself (silence excluded) after levelling a Gemini take, in dBFS.
pub const SPEECH_TARGET_DB: f64 = -19.0;
const EDGE_HELPER: &str = include_str!("../py/edge_voice.py");

#[derive(Debug, Clone)]
pub struct Cfg {
    pub engine: String,
    pub voice: String,
    pub rate: String,
    pub pitch: String,
    pub style: String,
    pub lang: String,
    pub model: String,
    /// Silence before the voice starts in a scene, and after it ends.
    pub lead: f64,
    pub tail: f64,
}

pub fn cfg(spec: &Value) -> Result<Cfg> {
    let v = &spec["voice"];
    let s = |k: &str, d: &str| v[k].as_str().unwrap_or(d).to_string();
    let beat_grid = spec["bpm"].is_number();
    let lang = s("lang", "en-US");
    let engine = s("engine", "edge");
    if !matches!(engine.as_str(), "edge" | "gemini") {
        return Err(format!("voice.engine `{engine}` must be `edge` (free) or `gemini`").into());
    }
    let default_voice = match (engine.as_str(), lang.starts_with("id")) {
        ("edge", true) => "id-ID-ArdiNeural",
        ("edge", false) => "en-US-EmmaMultilingualNeural",
        _ => "Kore",
    };
    let c = Cfg {
        engine,
        voice: s("voice", default_voice),
        rate: s("rate", "+0%"),
        pitch: s("pitch", "+0Hz"),
        style: s("style", ""),
        lang,
        model: gemini::normalise_model(&s("model", GEMINI_TTS_MODEL)),
        // on a beat grid the voice starts on the downbeat and the next beat supplies the breathing room
        lead: v["lead"]
            .as_f64()
            .unwrap_or(if beat_grid { 0.0 } else { 0.15 })
            .clamp(0.0, 5.0),
        tail: v["tail"]
            .as_f64()
            .unwrap_or(if beat_grid { 0.1 } else { 0.35 })
            .clamp(0.0, 5.0),
    };
    if c.engine == "gemini" {
        gemini::guard_model(&c.model)?;
        if !GEMINI_VOICES.contains(&c.voice.as_str()) {
            return Err(format!(
                "`{}` is not a Gemini voice. Choose one of: {}",
                c.voice,
                GEMINI_VOICES.join(", ")
            )
            .into());
        }
    }
    Ok(c)
}

/// The scene's own delivery: `voiceStyle` (how it is said) and `voiceLang` (BCP-47, for a line in
/// another language than the rest, such as an Indonesian tagline in an English video).
pub fn for_scene(base: &Cfg, scene: &Value) -> Cfg {
    let mut c = base.clone();
    if let Some(s) = scene["voiceStyle"].as_str() {
        c.style = s.to_string();
    }
    if let Some(l) = scene["voiceLang"].as_str() {
        c.lang = l.to_string();
    }
    c
}

pub fn hash(c: &Cfg, text: &str) -> String {
    let parts: Vec<&[u8]> = [
        c.engine.as_str(),
        &c.voice,
        &c.rate,
        &c.pitch,
        &c.style,
        &c.lang,
        &c.model,
        text,
    ]
    .iter()
    .map(|s| s.as_bytes())
    .collect();
    format!("{:016x}", fnv1a(&parts))
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub index: usize,
    pub text: String,
    pub hash: String,
    pub file: String,
    pub duration: f64,
    pub words: Vec<subs::Word>,
    /// What a transcription of the take heard (gemini takes only), for the record.
    pub heard: Option<String>,
}

pub fn dir(root: &Path, name: &str) -> PathBuf {
    root.join("assets/voice").join(name)
}

pub fn load(root: &Path, name: &str) -> Vec<Entry> {
    let Ok(text) = fs::read_to_string(dir(root, name).join("timeline.json")) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    v["scenes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            Some(Entry {
                index: e["index"].as_u64()? as usize,
                text: e["text"].as_str()?.to_string(),
                hash: e["hash"].as_str()?.to_string(),
                file: e["file"].as_str()?.to_string(),
                duration: e["duration"].as_f64()?,
                heard: e["heard"].as_str().map(str::to_string),
                words: e["words"]
                    .as_array()?
                    .iter()
                    .filter_map(|w| {
                        Some(subs::Word {
                            w: w["w"].as_str()?.to_string(),
                            s: w["s"].as_f64()?,
                            e: w["e"].as_f64()?,
                        })
                    })
                    .collect(),
            })
        })
        .collect()
}

pub fn save(root: &Path, name: &str, c: &Cfg, entries: &[Entry]) -> Result<()> {
    let d = dir(root, name);
    fs::create_dir_all(&d)?;
    let v = json!({
        "engine": c.engine, "voice": c.voice, "lang": c.lang,
        "about": "Generated by `rsepuh voice`. Times are seconds from the start of each clip.",
        "scenes": entries.iter().map(|e| json!({
            "index": e.index, "text": e.text, "hash": e.hash, "file": e.file, "heard": e.heard,
            "duration": (e.duration * 1000.0).round() / 1000.0,
            "words": e.words.iter().map(|w| json!({
                "w": w.w, "s": (w.s * 1000.0).round() / 1000.0, "e": (w.e * 1000.0).round() / 1000.0
            })).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    });
    fs::write(
        d.join("timeline.json"),
        serde_json::to_string_pretty(&v)? + "\n",
    )?;
    Ok(())
}

/// (scene index, spoken text) for every scene that has a `voice` line.
pub fn lines(spec: &Value) -> Vec<(usize, String)> {
    spec["scenes"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, s)| {
            let t = s["voice"]
                .as_str()?
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            (!t.is_empty()).then_some((i, t))
        })
        .collect()
}

// ------------------------------------------------------------------ engines

fn edge_command() -> Result<Command> {
    if Command::new("uv")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        let mut c = Command::new("uv");
        c.args([
            "run",
            "--no-project",
            "--quiet",
            "--with",
            "edge-tts",
            "python",
        ]);
        return Ok(c);
    }
    let ok = Command::new("python3")
        .args(["-c", "import edge_tts"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if ok {
        return Ok(Command::new("python3"));
    }
    Err("the free voice needs `uv` (https://docs.astral.sh/uv/, recommended) or `pip install -U edge-tts`. \
         Or use voice.engine \"gemini\" with a Gemini key"
        .into())
}

fn synth_edge(c: &Cfg, text: &str, out_mp3: &Path) -> Result<(f64, Vec<subs::Word>)> {
    let tmp = std::env::temp_dir().join(format!("rsepuh-edge-{}", std::process::id()));
    fs::create_dir_all(&tmp)?;
    let (helper, meta, mp3) = (
        tmp.join("edge_voice.py"),
        tmp.join("meta.json"),
        tmp.join("clip.mp3"),
    );
    fs::write(&helper, EDGE_HELPER)?;
    let result = (|| -> Result<(f64, Vec<subs::Word>)> {
        let mut child = edge_command()?
            .arg(&helper)
            .args([&mp3, &meta])
            .arg(&c.voice)
            .arg(&c.rate)
            .arg(&c.pitch)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        child
            .stdin
            .take()
            .ok_or("no stdin")?
            .write_all(text.as_bytes())?;
        let out = child.wait_with_output()?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let last = err
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("edge-tts failed");
            return Err(format!(
                "edge-tts failed: {last}{}",
                if err.contains("403") {
                    " (Microsoft rejects old edge-tts releases: run through `uv`, or `pip install -U edge-tts`)"
                } else {
                    ""
                }
            )
            .into());
        }
        let bytes = fs::read(&mp3)?;
        let m: Value = serde_json::from_slice(&fs::read(&meta)?)?;
        let spoken: Vec<(String, f64, f64)> = m["words"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| {
                let (o, d) = (w["offset"].as_f64()?, w["duration"].as_f64()?);
                Some((w["text"].as_str()?.to_string(), o, o + d))
            })
            .collect();
        if spoken.is_empty() {
            return Err("edge-tts returned audio without word times".into());
        }
        let last_end = spoken.last().map_or(0.0, |w| w.2);
        // edge pads the end of the clip with silence; keep a short natural tail instead
        let dur = audio::mp3_duration(&bytes)
            .unwrap_or(last_end + 0.2)
            .min(last_end + 0.18)
            .max(last_end);
        fs::write(out_mp3, &bytes)?;
        Ok((dur, subs::align(text, &spoken)))
    })();
    let _ = fs::remove_dir_all(&tmp);
    result
}

fn mime_param(mime: &str, key: &str) -> Option<u32> {
    mime.split(';').find_map(|p| {
        p.trim()
            .strip_prefix(&format!("{key}="))
            .and_then(|v| v.parse().ok())
    })
}

/// Direction goes in director's notes, which the model follows and does not read aloud. A
/// "Say cheerfully:" prefix or a bracketed tag is read out loud (verified by transcribing takes).
pub fn framed(style: &str, text: &str) -> String {
    if style.trim().is_empty() {
        text.to_string()
    } else {
        format!(
            "### DIRECTOR'S NOTES\nStyle: {}\n\n### TRANSCRIPT\n{text}",
            style.trim()
        )
    }
}

pub fn gemini_body(c: &Cfg, text: &str) -> Value {
    let mut speech = json!({"voiceConfig": {"prebuiltVoiceConfig": {"voiceName": c.voice}}});
    if !c.lang.is_empty() {
        speech["languageCode"] = json!(c.lang);
    }
    json!({
        "contents": [{"parts": [{"text": framed(&c.style, text)}]}],
        "generationConfig": {"responseModalities": ["AUDIO"], "speechConfig": speech}
    })
}

pub const TRANSCRIBE_MODEL: &str = "gemini-3.8-flash";

fn words_of(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect::<String>()
        })
        .filter(|w: &String| !w.is_empty())
        .collect()
}

/// Word-level edit distance between what the script says and what was heard.
pub fn word_distance(script: &str, heard: &str) -> usize {
    let (a, b) = (words_of(script), words_of(heard));
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, wa) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, wb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(wa != wb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// How far (0 = identical) what was heard is from the script, as the share of letters that
/// differ, ignoring case, punctuation and word breaks: "Effect Craft" for "EffectCraft" is fine,
/// a read-aloud direction or a dropped word is not.
pub fn mismatch(script: &str, heard: &str) -> f64 {
    let a: Vec<char> = words_of(script).concat().chars().collect();
    let b: Vec<char> = words_of(heard).concat().chars().collect();
    if a.is_empty() {
        return if b.is_empty() { 0.0 } else { 1.0 };
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()] as f64 / a.len() as f64
}

/// A take is accepted when at most this share of its letters differ from the script.
pub const MAX_MISMATCH: f64 = 0.12;

/// The direction to try on each attempt: the full direction twice, then only its first two
/// phrases, then none. Some directions get read aloud by the model; a shorter one or none works.
pub fn direction_ladder(style: &str) -> Vec<String> {
    let phrases: Vec<&str> = style
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let short = phrases
        .iter()
        .take(2)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    vec![style.to_string(), style.to_string(), short, String::new()]
}

/// Transcribe a clip with the text model, so a take that added, dropped or misread a word is
/// caught before it goes into the edit. Returns the heard text.
pub fn transcribe(key: &str, wav: &[u8]) -> Result<String> {
    let body = json!({"contents": [{"parts": [
        {"inline_data": {"mime_type": "audio/wav", "data": crate::util::base64(wav)}},
        {"text": "Transcribe this audio verbatim, including any instructions or stage directions that are spoken aloud. Output only the transcript."}
    ]}]});
    let resp = gemini::post_json(key, TRANSCRIBE_MODEL, &body)?;
    resp["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(|t| t.trim().to_string())
        .ok_or_else(|| "the transcription model returned no text".into())
}

fn synth_gemini(c: &Cfg, text: &str, key: &str, out_wav: &Path) -> Result<(f64, Vec<subs::Word>)> {
    let resp = gemini::post_json(key, &c.model, &gemini_body(c, text))?;
    let (mime, bytes) = gemini::extract_audio(&resp)?;
    let (rate, ch, pcm): (u32, u16, Vec<u8>) = if let Some((r, ch, d)) = audio::parse_wav(&bytes) {
        (r, ch, d.to_vec())
    } else if mime.to_ascii_lowercase().contains("l16") || mime.contains("pcm") {
        (
            mime_param(&mime, "rate").unwrap_or(24000),
            mime_param(&mime, "channels").unwrap_or(1) as u16,
            bytes,
        )
    } else {
        return Err(format!(
            "Gemini TTS returned `{mime}`, which rsepuh does not read (WAV or raw 16-bit PCM)"
        )
        .into());
    };
    if ch != 1 {
        return Err("Gemini TTS returned multi-channel audio; expected mono".into());
    }
    let (a, b) = audio::speech_range(&pcm, rate, -45.0).ok_or("Gemini TTS returned silence")?;
    // every take at the same speech level, so no scene is louder than another
    let (leveled, _) = audio::normalize_active(&pcm[a..b], rate, SPEECH_TARGET_DB, -1.5);
    let kept = &leveled[..];
    fs::write(out_wav, audio::wav_from_pcm16(kept, rate, 1))?;
    let dur = audio::pcm_seconds(kept.len(), rate, 1);
    // anchor the words on the real pauses of this clip, so punctuation lines up with the sound
    let pauses = audio::find_pauses(kept, rate, -42.0, 0.08);
    let span = (0.03_f64.min(dur / 4.0), (dur - 0.06).max(dur / 2.0));
    let words = subs::anchored(text, span, &pauses);
    Ok((dur, words))
}

/// Generate (or keep, when unchanged) the clip of every scene that has a `voice` line.
pub fn synth(
    root: &Path,
    name: &str,
    spec: &Value,
    force: bool,
    env_file: Option<&Path>,
) -> Result<Vec<String>> {
    let c = cfg(spec)?;
    let todo = lines(spec);
    if todo.is_empty() {
        return Err(
            "no scene has a `voice` line (add \"voice\": \"what is said\" to a scene)".into(),
        );
    }
    let key =
        if c.engine == "gemini" {
            Some(gemini::find_key(env_file).ok_or(
                "Gemini TTS needs GEMINI_API_KEY or GEMINI_KEY (env, --env FILE or ./.env)",
            )?)
        } else {
            None
        };
    let existing = load(root, name);
    let d = dir(root, name);
    fs::create_dir_all(&d)?;
    let (mut entries, mut log) = (Vec::new(), Vec::new());
    for (i, text) in todo {
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(format!(
                "scene {} voice line is {} characters (limit {MAX_TEXT_CHARS}); split the scene",
                i + 1,
                text.chars().count()
            )
            .into());
        }
        let sc_cfg = for_scene(&c, &spec["scenes"][i]);
        let h = hash(&sc_cfg, &text);
        if let Some(e) = existing
            .iter()
            .find(|e| e.index == i && e.hash == h && !force)
        {
            if root.join(&e.file).is_file() {
                log.push(format!("scene {}: kept ({:.2}s)", i + 1, e.duration));
                entries.push(e.clone());
                continue;
            }
        }
        let ext = if c.engine == "edge" { "mp3" } else { "wav" };
        let rel = format!("assets/voice/{name}/s{i:02}.{ext}");
        // Gemini takes are stochastic: transcribe each one against the script and retake when it
        // added, dropped or misread words (for example read the direction aloud), loosening the
        // direction on later attempts.
        let mut heard: Option<String> = None;
        let mut notes = String::new();
        let (duration, words) = match &key {
            None => synth_edge(&sc_cfg, &text, &root.join(&rel))?,
            Some(k) => {
                let mut best: Option<(f64, f64, Vec<subs::Word>, String)> = None;
                let ladder = direction_ladder(&sc_cfg.style);
                for (attempt, direction) in ladder.iter().enumerate() {
                    let mut take = sc_cfg.clone();
                    take.style = direction.clone();
                    let (d, w) = synth_gemini(&take, &text, k, &root.join(&rel))
                        .map_err(|e| gemini::redact(&e.to_string(), k))?;
                    let wav = fs::read(root.join(&rel))?;
                    let said =
                        transcribe(k, &wav).map_err(|e| gemini::redact(&e.to_string(), k))?;
                    let m = mismatch(&text, &said);
                    if best.as_ref().is_none_or(|b| m < b.0) {
                        fs::copy(root.join(&rel), root.join(format!("{rel}.best")))?;
                        best = Some((m, d, w, said.clone()));
                    }
                    if m <= MAX_MISMATCH {
                        if attempt > 0 {
                            notes += &format!(
                                " (take {}: {})",
                                attempt + 1,
                                if direction.is_empty() {
                                    "direction dropped".into()
                                } else {
                                    format!("direction \"{direction}\"")
                                }
                            );
                        }
                        break;
                    }
                    notes += &format!(" [take {} heard \"{said}\"]", attempt + 1);
                }
                let (m, d, w, said) = best.ok_or("no take")?;
                fs::rename(root.join(format!("{rel}.best")), root.join(&rel))?;
                if m > MAX_MISMATCH {
                    notes += &format!(
                        " WARNING: best take still differs from the script ({:.0}% of letters)",
                        m * 100.0
                    );
                }
                heard = Some(said);
                (d, w)
            }
        };
        log.push(format!(
            "scene {}: {} words, {:.2}s ({}, {}){notes}",
            i + 1,
            words.len(),
            duration,
            c.engine,
            c.voice
        ));
        entries.push(Entry {
            index: i,
            text,
            hash: h,
            file: rel,
            duration,
            words,
            heard,
        });
    }
    save(root, name, &c, &entries)?;
    Ok(log)
}

/// Voices an engine offers. Gemini's list is fixed; edge asks the service (needs network).
pub fn list_voices(engine: &str, lang: &str) -> Result<Vec<String>> {
    if engine == "gemini" {
        return Ok(GEMINI_VOICES.iter().map(|s| s.to_string()).collect());
    }
    let code = "import asyncio,sys,edge_tts\n\
                async def m():\n\
                \tfor v in await edge_tts.list_voices():\n\
                \t\tif v['Locale'].lower().startswith(sys.argv[1].lower()): print(v['ShortName'], v['Gender'], v['Locale'])\n\
                asyncio.run(m())\n";
    let out = edge_command()?.args(["-c", code, lang]).output()?;
    if !out.status.success() {
        return Err(format!(
            "edge-tts: {}",
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("failed")
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_follow_language_and_engine() {
        let c = cfg(&json!({"voice": {"lang": "id-ID"}})).unwrap();
        assert_eq!(
            (c.engine.as_str(), c.voice.as_str()),
            ("edge", "id-ID-ArdiNeural")
        );
        let c = cfg(&json!({})).unwrap();
        assert_eq!(c.voice, "en-US-EmmaMultilingualNeural");
        let c = cfg(&json!({"voice": {"engine": "gemini"}})).unwrap();
        assert_eq!(
            (c.voice.as_str(), c.model.as_str()),
            ("Kore", GEMINI_TTS_MODEL)
        );
        assert!(cfg(&json!({"voice": {"engine": "azure"}})).is_err());
        assert!(
            cfg(&json!({"voice": {"engine": "gemini", "voice": "Nope"}}))
                .unwrap_err()
                .to_string()
                .contains("not a Gemini voice")
        );
        assert!(cfg(&json!({"voice": {"engine": "gemini", "model": "veo-3"}})).is_err());
    }

    #[test]
    fn hash_changes_with_text_voice_or_settings() {
        let a = cfg(&json!({})).unwrap();
        let mut b = a.clone();
        b.rate = "+10%".into();
        assert_ne!(hash(&a, "hi"), hash(&a, "hi!"));
        assert_ne!(hash(&a, "hi"), hash(&b, "hi"));
        assert_eq!(hash(&a, "hi"), hash(&a.clone(), "hi"));
    }

    #[test]
    fn timeline_round_trips_and_lines_skip_scenes_without_voice() {
        let root = std::env::temp_dir().join(format!("rsepuh-voice-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let c = cfg(&json!({})).unwrap();
        let e = Entry {
            index: 2,
            text: "halo dunia".into(),
            hash: hash(&c, "halo dunia"),
            file: "assets/voice/x/s02.mp3".into(),
            duration: 1.25,
            words: subs::distribute("halo dunia", 0.1, 1.0),
            heard: None,
        };
        save(&root, "x", &c, std::slice::from_ref(&e)).unwrap();
        let back = load(&root, "x");
        assert_eq!(back.len(), 1);
        assert_eq!(
            (back[0].index, back[0].file.as_str(), back[0].words.len()),
            (2, "assets/voice/x/s02.mp3", 2)
        );
        assert!((back[0].duration - 1.25).abs() < 1e-9);
        let spec = json!({"scenes": [{"voice": " a  b "}, {"type": "hook"}, {"voice": ""}, {"voice": "c"}]});
        assert_eq!(
            lines(&spec),
            vec![(0, "a b".to_string()), (3, "c".to_string())]
        );
        assert!(load(&root, "missing").is_empty());
    }

    #[test]
    fn mismatch_ignores_case_punctuation_and_word_breaks_but_catches_read_aloud_directions() {
        assert!(mismatch("Edit like a veteran.", "edit, like a Veteran") < 1e-9);
        assert!(
            mismatch(
                "EffectCraft. Blender. Gemini.",
                "Effect Craft, Blender, Gemini!"
            ) < 1e-9
        );
        assert!(mismatch("Motion, 3D, and stills. One command.", "Director's notes: Bright, energetic, quick pace. Motion, 3D, and stills. One command.") > MAX_MISMATCH);
        assert!(mismatch("Edit like a veteran.", "Edit like a Yasapu") > MAX_MISMATCH);
        assert!(
            mismatch("a b c d e f g h", "a b c d e f g") <= MAX_MISMATCH + 0.01,
            "one dropped letter in eight is within tolerance"
        );
        assert_eq!(mismatch("", ""), 0.0);
    }

    #[test]
    fn direction_loosens_on_later_takes() {
        let l = direction_ladder("bright, energetic, quick pace");
        assert_eq!(
            l,
            [
                "bright, energetic, quick pace",
                "bright, energetic, quick pace",
                "bright, energetic",
                ""
            ]
        );
        assert_eq!(direction_ladder("")[3], "");
    }

    #[test]
    fn gemini_request_names_voice_language_and_audio_modality() {
        let mut c = cfg(&json!({"voice": {"engine": "gemini", "voice": "Puck", "lang": "id-ID", "style": "upbeat"}})).unwrap();
        let b = gemini_body(&c, "Halo");
        assert_eq!(b["generationConfig"]["responseModalities"][0], "AUDIO");
        assert_eq!(
            b["generationConfig"]["speechConfig"]["voiceConfig"]["prebuiltVoiceConfig"]
                ["voiceName"],
            "Puck"
        );
        assert_eq!(
            b["generationConfig"]["speechConfig"]["languageCode"],
            "id-ID"
        );
        assert_eq!(
            b["contents"][0]["parts"][0]["text"],
            "### DIRECTOR'S NOTES\nStyle: upbeat\n\n### TRANSCRIPT\nHalo"
        );
        c.style.clear();
        assert_eq!(
            gemini_body(&c, "Halo")["contents"][0]["parts"][0]["text"],
            "Halo"
        );
        assert_eq!(
            mime_param("audio/L16;codec=pcm;rate=24000", "rate"),
            Some(24000)
        );
    }
}
