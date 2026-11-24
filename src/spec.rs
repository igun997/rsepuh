//! The project spec (`scenes/NAME.json`): validation, theme resolution, asset checks, and the
//! generated EffectCraft script (spec + theme + engine) that the remote runs.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};

use crate::assets;
use crate::subs::{self, Cue, Word};
use crate::synth::Sfx;
use crate::voice;
use crate::Result;

pub const SCENE_TYPES: &[&str] = &[
    "hook", "title", "media", "chips", "terminal", "stat", "caption", "cta", "code", "lottie",
    "custom",
];

pub struct Built {
    pub name: String,
    /// The compositor script with the spec and style injected.
    pub jsx: String,
    /// The resolved spec and style as JSON, for the motion writer (lottie/writer.py).
    pub plan: String,
    pub scene_count: usize,
    pub total: f64,
    /// One settled moment per scene, used for the contact sheet.
    pub frame_times: Vec<f64>,
    pub warnings: Vec<String>,
    /// Subtitle cues with absolute times, for SRT/VTT export.
    pub cues: Vec<Cue>,
    pub bpm: Option<f64>,
    /// Sound effect events (seconds, kind) derived from cuts and on-screen hits.
    pub sfx: Vec<(f64, Sfx)>,
    /// Style name when the spec asks for a generated music bed (`music.gen`).
    pub music_gen: Option<String>,
}

/// Round `x` up to a multiple of `unit` (a beat or a bar), ignoring float noise.
pub fn snap_up(x: f64, unit: f64) -> f64 {
    ((x / unit) - 1e-6).ceil().max(1.0) * unit
}

fn snap_near(x: f64, unit: f64) -> f64 {
    (x / unit).round() * unit
}

/// Beat-grid timing of the moments inside a scene (seconds from the scene start), so pops and
/// word hits land on the kick instead of wherever the layout put them.
fn plan_times(ty: &str, sc: &Value, dur: f64, beat: f64) -> Value {
    let half = beat / 2.0;
    match ty {
        "hook" => {
            let n = sc["words"].as_array().map(Vec::len).unwrap_or_else(|| {
                sc["text"]
                    .as_str()
                    .map_or(1, |t| t.split_whitespace().count().max(1))
            });
            let raw = if n > 1 { dur * 0.78 / n as f64 } else { dur };
            let mut t: Vec<f64> = Vec::new();
            for k in 0..n {
                let mut v = snap_near(k as f64 * raw, half);
                if let Some(prev) = t.last() {
                    v = v.max(prev + half);
                }
                t.push(v.min(dur - half));
            }
            json!({ "wordT": t })
        }
        "chips" => {
            let step = if beat >= 0.45 { beat } else { beat * 2.0 };
            let n = sc["items"].as_array().map_or(0, Vec::len);
            let t: Vec<f64> = (0..n)
                .map(|k| (step * (1 + k) as f64).min(dur - half))
                .collect();
            json!({ "itemT": t })
        }
        "cta" => {
            json!({ "buttonT": (beat * 2.0).min(dur * 0.6), "urlT": (beat * 3.0).min(dur * 0.8) })
        }
        "media" => json!({ "captionT": beat.min(dur * 0.4) }),
        "stat" => json!({ "countDur": (beat * 2.0).min(dur * 0.7) }),
        _ => json!({}),
    }
}

fn norm_word(w: &str) -> String {
    w.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Find a spoken word by name (`word` or `word#2` for the second occurrence) at or after
/// `from` in the scene's words. Returns its index.
fn find_word(words: &[Word], target: &str, from: usize) -> Option<usize> {
    let (name, nth) = match target.split_once('#') {
        Some((n, k)) => (n, k.parse::<usize>().unwrap_or(1).max(1)),
        None => (target, 1),
    };
    let want = norm_word(name);
    words
        .iter()
        .enumerate()
        .skip(from)
        .filter(|(_, w)| norm_word(&w.w) == want)
        .nth(nth - 1)
        .map(|(i, _)| i)
}

/// Pin on-screen moments to spoken words. `onWord` maps a field to a word, `onWords` maps a
/// field to a list of words (found in order). Fields ending in `T` become seconds from the scene
/// start, `Beat`/`Beats` fields become beats. `focus[].atWord` / `untilWord` do the same for
/// code-focus entries. This is what keeps a pop or a highlight on the word that explains it.
fn apply_word_anchors(
    sc: &mut Value,
    words: &[Word],
    at: f64,
    beat: f64,
    scene_no: usize,
) -> Result<()> {
    let has = sc["onWord"].is_object()
        || sc["onWords"].is_object()
        || sc["focus"]
            .as_array()
            .is_some_and(|f| f.iter().any(|x| x["atWord"].is_string()));
    if !has {
        return Ok(());
    }
    if words.is_empty() {
        return Err(format!("scene {scene_no} uses onWord/onWords/atWord but has no voice line to take word times from").into());
    }
    let value = |key: &str, w: &Word| -> f64 {
        let rel = w.s - at;
        let v = if key.ends_with("Beat") || key.ends_with("Beats") {
            rel / beat
        } else {
            rel
        };
        (v * 1000.0).round() / 1000.0
    };
    let missing = |t: &str| -> Box<dyn std::error::Error> {
        let heard: Vec<&str> = words.iter().map(|w| w.w.as_str()).collect();
        format!(
            "scene {scene_no}: the word `{t}` is not spoken (voice says: {})",
            heard.join(" ")
        )
        .into()
    };
    if let Some(m) = sc["onWord"].as_object().cloned() {
        for (k, t) in m {
            let t = t.as_str().ok_or("onWord values are words")?;
            let i = find_word(words, t, 0).ok_or_else(|| missing(t))?;
            sc[k.as_str()] = json!(value(&k, &words[i]));
        }
    }
    if let Some(m) = sc["onWords"].as_object().cloned() {
        for (k, list) in m {
            let (mut from, mut out) = (0usize, Vec::new());
            for t in list.as_array().ok_or("onWords values are lists of words")? {
                let t = t.as_str().ok_or("onWords entries are words")?;
                let i = find_word(words, t, from).ok_or_else(|| missing(t))?;
                out.push(value(&k, &words[i]));
                from = i + 1;
            }
            sc[k.as_str()] = json!(out);
        }
    }
    if let Some(focus) = sc["focus"].as_array().cloned() {
        let (mut from, mut out) = (0usize, Vec::new());
        for mut f in focus {
            if let Some(t) = f["atWord"].as_str() {
                let i = find_word(words, t, from).ok_or_else(|| missing(t))?;
                f["atBeat"] = json!(value("atBeat", &words[i]));
                from = i + 1;
                let until = match f["untilWord"].as_str() {
                    Some(u) => {
                        let j = find_word(words, u, from).ok_or_else(|| missing(u))?;
                        from = j;
                        value("untilBeat", &words[j])
                    }
                    None => f["atBeat"].as_f64().unwrap_or(0.0) + 4.0,
                };
                f["untilBeat"] = json!(until);
            }
            out.push(f);
        }
        sc["focus"] = Value::Array(out);
    }
    Ok(())
}

fn sfx_for(ty: &str, sc: &Value, at: f64, beat: f64, first: bool, out: &mut Vec<(f64, Sfx)>) {
    // every transition but `cut` is a move into the cut: a whoosh builds, an impact lands on it
    let tr = sc["transition"].as_str();
    if tr != Some("cut") {
        if !first {
            out.push((at, Sfx::Whoosh));
        }
        out.push((at, Sfx::Impact));
    }
    let rel = |key: &str| -> Vec<f64> {
        sc[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_f64)
            .collect()
    };
    match ty {
        "hook" => out.extend(rel("wordT").into_iter().map(|t| (at + t, Sfx::Tick))),
        "chips" => out.extend(rel("itemT").into_iter().map(|t| (at + t, Sfx::Pop))),
        "cta" => out.extend(sc["buttonT"].as_f64().map(|t| (at + t, Sfx::Pop))),
        "media" => out.extend(sc["captionT"].as_f64().map(|t| (at + t, Sfx::Pop))),
        // custom scenes name their moments in beats: `*Beat` numbers and `*Beats` arrays pop,
        // `focus[].atBeat` ticks (`startBeat` only says when typing begins)
        "custom" => {
            for (k, v) in sc.as_object().into_iter().flatten() {
                if k.ends_with("Beat") && k != "startBeat" {
                    out.extend(v.as_f64().map(|b| (at + b * beat, Sfx::Pop)));
                } else if k.ends_with("Beats") {
                    for b in v.as_array().into_iter().flatten().filter_map(Value::as_f64) {
                        out.push((at + b * beat, Sfx::Pop));
                    }
                } else if k == "focus" {
                    for f in v.as_array().into_iter().flatten() {
                        out.extend(f["atBeat"].as_f64().map(|b| (at + b * beat, Sfx::Tick)));
                    }
                }
            }
        }
        _ => {}
    }
}

/// Safe file stem: lowercase ascii, digits, `-` and `_`.
pub fn slug(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "video".into()
    } else {
        out
    }
}

pub fn aspect_size(aspect: &str) -> Option<(u32, u32)> {
    Some(match aspect {
        "9:16" => (1080, 1920),
        "16:9" => (1920, 1080),
        "1:1" => (1080, 1080),
        "4:5" => (1080, 1350),
        "3:4" => (1080, 1440),
        _ => return None,
    })
}

/// A project-relative path that cannot leave the project (the remote only sees synced files).
fn safe_rel(p: &str) -> Result<PathBuf> {
    let path = Path::new(p);
    if path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(format!(
            "`{p}` must be a path inside the project (no absolute paths or `..`): the remote only sees synced project files"
        )
        .into());
    }
    if p.contains('"') || p.contains('\\') || p.contains('\n') {
        return Err(
            format!("`{p}` contains a character that is not allowed in asset paths").into(),
        );
    }
    Ok(path.to_path_buf())
}

/// `%04d` or a run of `#` in a file name stands for a frame number. Returns
/// (prefix, digits, suffix).
fn seq_pattern(name: &str) -> Option<(String, usize, String)> {
    if let Some(i) = name.find('%') {
        let rest = &name[i + 1..];
        let zeros = rest.starts_with('0');
        let digits: String = rest
            .trim_start_matches('0')
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let after = rest.trim_start_matches('0').trim_start_matches(&digits[..]);
        if zeros && !digits.is_empty() && after.starts_with('d') {
            return Some((
                name[..i].to_string(),
                digits.parse().ok()?,
                after[1..].to_string(),
            ));
        }
    }
    let first = name.find('#')?;
    let n = name[first..].chars().take_while(|c| *c == '#').count();
    Some((name[..first].to_string(), n, name[first + n..].to_string()))
}

/// Resolve one asset path. For sequences returns the first existing frame and `true`.
fn resolve_asset(root: &Path, rel: &str) -> Result<(String, bool)> {
    let rel_path = safe_rel(rel)?;
    let name = rel_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some((prefix, digits, suffix)) = seq_pattern(&name) {
        let dir = root.join(rel_path.parent().unwrap_or(Path::new("")));
        let mut best: Option<(u64, String)> = None;
        for e in fs::read_dir(&dir)
            .map_err(|e| {
                format!(
                    "image sequence folder {} is not readable: {e}",
                    dir.display()
                )
            })?
            .flatten()
        {
            let n = e.file_name().to_string_lossy().into_owned();
            if let Some(mid) = n
                .strip_prefix(&prefix)
                .and_then(|r| r.strip_suffix(&suffix))
            {
                if mid.len() == digits && mid.chars().all(|c| c.is_ascii_digit()) {
                    let v: u64 = mid.parse().unwrap_or(u64::MAX);
                    if best.as_ref().is_none_or(|(b, _)| v < *b) {
                        best = Some((v, n));
                    }
                }
            }
        }
        let (_, first) = best.ok_or_else(|| format!("no frames match `{rel}`"))?;
        let parent = rel_path.parent().unwrap_or(Path::new(""));
        return Ok((parent.join(first).to_string_lossy().into_owned(), true));
    }
    if !root.join(&rel_path).is_file() {
        return Err(format!(
            "asset `{rel}` does not exist. Generate it first (rsepuh gen image, rsepuh blender) or fix the path"
        )
        .into());
    }
    Ok((rel_path.to_string_lossy().into_owned(), false))
}

pub fn builtin_theme(name: &str) -> Option<Value> {
    assets::THEMES
        .iter()
        .find(|(n, _)| *n == name)
        .and_then(|(_, body)| serde_json::from_str(body).ok())
}

fn load_theme(root: &Path, spec: &Value) -> Result<Value> {
    let name = spec["style"].as_str().unwrap_or("punch");
    let mut theme = if let Some(t) = builtin_theme(name) {
        t
    } else {
        let p = root.join("styles").join(format!("{}.json", slug(name)));
        let text = fs::read_to_string(&p).map_err(|_| {
            format!(
                "unknown style `{name}`. Built in: {}. Or add styles/{}.json",
                assets::THEMES
                    .iter()
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", "),
                slug(name)
            )
        })?;
        let mut custom: Value = serde_json::from_str(&text)
            .map_err(|e| format!("styles/{}.json is not valid JSON: {e}", slug(name)))?;
        // A custom style starts from `punch` so it only needs to name what it changes.
        let mut base = builtin_theme("punch").unwrap_or_else(|| json!({}));
        merge(&mut base, custom.take());
        base
    };
    if let Some(over) = spec.get("theme") {
        merge(&mut theme, over.clone());
    }
    Ok(theme)
}

fn merge(base: &mut Value, over: Value) {
    if let (Some(b), Value::Object(o)) = (base.as_object_mut(), over) {
        for (k, v) in o {
            b.insert(k, v);
        }
    }
}

fn need(sc: &Value, i: usize, keys: &[&str]) -> Result<()> {
    if keys.iter().any(|k| !sc[*k].is_null()) {
        return Ok(());
    }
    Err(format!(
        "scene {} (`{}`) needs {}",
        i + 1,
        sc["type"].as_str().unwrap_or("?"),
        keys.iter()
            .map(|k| format!("`{k}`"))
            .collect::<Vec<_>>()
            .join(" or ")
    )
    .into())
}

pub const SUBTITLE_STYLES: &[&str] = &[
    "karaoke",
    "pop",
    "box",
    "outline",
    "typewriter",
    "marker",
    "lower",
    "glow",
];

fn num(v: &Value, k: &str, d: f64, lo: f64, hi: f64) -> f64 {
    v[k].as_f64().unwrap_or(d).clamp(lo, hi)
}

/// Validate the spec and produce the script the remote executes. Scene lengths, voice audio and
/// subtitle cues are all resolved here from one timeline, so they cannot disagree.
pub fn build(root: &Path, spec_path: &Path) -> Result<Built> {
    build_opts(root, spec_path, true)
}

/// `require_generated: false` lets `rsepuh music` compute the timeline before the music and
/// sound effect files it is about to write exist.
pub fn build_opts(root: &Path, spec_path: &Path, require_generated: bool) -> Result<Built> {
    let text = fs::read_to_string(spec_path)
        .map_err(|e| format!("cannot read {}: {e}", spec_path.display()))?;
    let mut spec: Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not valid JSON: {e}", spec_path.display()))?;
    let mut warnings = Vec::new();

    let stem = spec_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = slug(spec["name"].as_str().unwrap_or(&stem));
    spec["name"] = json!(name);

    let (w, h) = if let Some(a) = spec["size"].as_array() {
        match (
            a.first().and_then(Value::as_u64),
            a.get(1).and_then(Value::as_u64),
        ) {
            (Some(w), Some(h)) if (64..=8192).contains(&w) && (64..=8192).contains(&h) => {
                (w as u32, h as u32)
            }
            _ => return Err("`size` must be [width, height] between 64 and 8192".into()),
        }
    } else {
        let a = spec["aspect"].as_str().unwrap_or("9:16");
        aspect_size(a).ok_or_else(|| format!("unknown aspect `{a}` (9:16, 16:9, 1:1, 4:5, 3:4)"))?
    };
    if w % 2 != 0 || h % 2 != 0 {
        return Err("width and height must be even (H.264)".into());
    }
    spec["size"] = json!([w, h]);
    let fps = spec["fps"].as_f64().unwrap_or(30.0);
    if !(1.0..=120.0).contains(&fps) {
        return Err("`fps` must be between 1 and 120".into());
    }
    spec["fps"] = json!(fps);
    let bpm = match spec["bpm"].as_f64() {
        Some(b) if (40.0..=220.0).contains(&b) => Some(b),
        Some(_) => return Err("`bpm` must be between 40 and 220".into()),
        None => None,
    };
    let beat = bpm.map(|b| 60.0 / b);
    let snap = spec["snap"]
        .as_str()
        .unwrap_or(if bpm.is_some() { "beat" } else { "off" });
    if !matches!(snap, "beat" | "bar" | "off") {
        return Err("`snap` must be beat, bar or off".into());
    }
    if snap != "off" && bpm.is_none() {
        return Err("`snap` needs a `bpm`".into());
    }

    let theme = load_theme(root, &spec)?;
    let scenes = spec["scenes"]
        .as_array()
        .filter(|s| !s.is_empty())
        .ok_or("the spec needs a non-empty `scenes` array")?
        .clone();

    let has_voice = !voice::lines(&spec).is_empty();
    let vcfg = if has_voice {
        Some(voice::cfg(&spec)?)
    } else {
        None
    };
    let timeline = if has_voice {
        voice::load(root, &name)
    } else {
        Vec::new()
    };

    let sub_cfg = spec["subtitles"].clone();
    let style = sub_cfg["style"].as_str().unwrap_or("karaoke").to_string();
    if !SUBTITLE_STYLES.contains(&style.as_str()) {
        return Err(format!(
            "unknown subtitle style `{style}` ({})",
            SUBTITLE_STYLES.join(", ")
        )
        .into());
    }
    let max_words = num(&sub_cfg, "maxWords", 4.0, 1.0, 12.0) as usize;
    let max_chars = num(&sub_cfg, "maxChars", 28.0, 8.0, 80.0) as usize;

    let mut out_scenes = Vec::new();
    let (mut total, mut frame_times) = (0.0f64, Vec::new());
    let (mut cues, mut windows): (Vec<Cue>, Vec<(f64, f64)>) = (Vec::new(), Vec::new());
    let mut sfx_events: Vec<(f64, Sfx)> = Vec::new();
    for (i, sc) in scenes.iter().enumerate() {
        let mut sc = sc.clone();
        let ty = sc["type"].as_str().unwrap_or("").to_string();
        if !SCENE_TYPES.contains(&ty.as_str()) {
            return Err(format!(
                "scene {}: unknown type `{ty}` (use {})",
                i + 1,
                SCENE_TYPES.join(", ")
            )
            .into());
        }
        match ty.as_str() {
            "hook" => need(&sc, i, &["words", "text"])?,
            "title" => need(&sc, i, &["lines", "text"])?,
            "chips" => need(&sc, i, &["items"])?,
            "terminal" => need(&sc, i, &["lines"])?,
            "stat" => need(&sc, i, &["value"])?,
            "caption" => need(&sc, i, &["text"])?,
            "cta" => need(&sc, i, &["title"])?,
            "media" => need(&sc, i, &["src", "overlay", "caption"])?,
            "code" => need(&sc, i, &["code"])?,
            "lottie" | "custom" => need(&sc, i, &["file"])?,
            _ => {}
        }
        for (key, seq_key) in [("src", "seq"), ("overlay", "overlaySeq")] {
            if let Some(p) = sc[key].as_str() {
                let (resolved, seq) = resolve_asset(root, p)?;
                sc[key] = json!(resolved);
                sc[seq_key] = json!(seq);
            }
        }
        if ty == "lottie" || ty == "custom" {
            // a Lottie file you authored, or (custom) a Python scene writer in lottie/scenes/
            let f = sc["file"].as_str().unwrap_or_default().to_string();
            let rel = safe_rel(&f)?;
            let ok_ext = if ty == "lottie" {
                f.ends_with(".json")
            } else {
                f.ends_with(".json") || f.ends_with(".py")
            };
            if !ok_ext {
                return Err(format!(
                    "scene {} `{f}`: a {ty} scene needs a {} file",
                    i + 1,
                    if ty == "lottie" {
                        ".json (Lottie)"
                    } else {
                        ".py (scene writer) or .json (Lottie)"
                    }
                )
                .into());
            }
            if !root.join(&rel).is_file() {
                return Err(format!("scene {} file `{f}` does not exist", i + 1).into());
            }
        }

        let at = sc["at"].as_f64().unwrap_or(total);
        let mut dur = sc["dur"].as_f64().unwrap_or(0.0);
        let mut scene_words: Vec<Word> = Vec::new();
        let voiced = voice::lines(&spec).into_iter().find(|(idx, _)| *idx == i);
        if let (Some((_, line)), Some(c)) = (&voiced, &vcfg) {
            let entry = timeline.iter().find(|e| e.index == i).ok_or_else(|| {
                format!(
                    "scene {} has a voice line but no clip yet: run `rsepuh voice {}`",
                    i + 1,
                    spec_path.display()
                )
            })?;
            if entry.hash != voice::hash(&voice::for_scene(c, &sc), line)
                || !root.join(&entry.file).is_file()
            {
                return Err(format!(
                    "the voice clip of scene {} is out of date (text or voice settings changed): run `rsepuh voice {}`",
                    i + 1,
                    spec_path.display()
                )
                .into());
            }
            dur = dur.max(c.lead + entry.duration + c.tail);
            let start = at + c.lead;
            sc["voiceFile"] = json!(entry.file);
            sc["voiceAt"] = json!(start);
            sc["voiceDur"] = json!(entry.duration);
            windows.push((start, start + entry.duration));
            scene_words = entry
                .words
                .iter()
                .map(|w| Word {
                    w: w.w.clone(),
                    s: start + w.s,
                    e: start + w.e,
                })
                .collect();
        } else if let Some(say) = sc["say"].as_str() {
            if dur >= 0.5 {
                scene_words = subs::distribute(say, at + 0.15, at + dur - 0.15);
            }
        }
        if !(0.1..=600.0).contains(&dur) {
            return Err(format!(
                "scene {} (`{ty}`) needs `dur` in seconds (0.1 to 600), or a `voice` line to take its length from",
                i + 1
            )
            .into());
        }
        if let (Some(b), true) = (beat, snap != "off") {
            dur = snap_up(dur, if snap == "bar" { b * 4.0 } else { b });
        }
        sc["dur"] = json!(dur);
        sc["at"] = json!(at);
        if let Some(b) = beat {
            if let Some(o) = plan_times(&ty, &sc, dur, b).as_object() {
                for (k, v) in o {
                    sc[k.as_str()] = v.clone();
                }
            }
            apply_word_anchors(&mut sc, &scene_words, at, b, i + 1)?;
            sfx_for(&ty, &sc, at, b, i == 0, &mut sfx_events);
        }
        if sc["subtitle"].as_bool() != Some(false) {
            cues.extend(subs::chunk(&scene_words, max_words, max_chars));
        }
        total = total.max(at + dur);
        frame_times.push(at + (dur * 0.7).min(dur - 0.05).max(0.0));
        out_scenes.push(sc);
    }
    if let Some(f) = sub_cfg["file"].as_str() {
        let rel = safe_rel(f)?;
        let text = fs::read_to_string(root.join(&rel))
            .map_err(|e| format!("subtitles file `{f}`: {e}"))?;
        let offset = sub_cfg["offset"].as_f64().unwrap_or(0.0);
        let parsed = subs::parse_cues(&text);
        if parsed.is_empty() {
            return Err(format!("subtitles file `{f}` has no cues (SRT or VTT expected)").into());
        }
        for (s, e, t) in parsed {
            let words = subs::distribute(&t, s + offset, e + offset);
            cues.extend(subs::chunk(&words, max_words.max(8), max_chars.max(60)));
        }
    }
    cues.sort_by(|a, b| a.s.total_cmp(&b.s));
    if let Some(d) = spec["duration"].as_f64() {
        if d < total - 0.001 {
            warnings.push(format!(
                "`duration` {d}s is shorter than the scenes ({total:.2}s); using {total:.2}s"
            ));
        }
        total = total.max(d);
    }
    spec["scenes"] = Value::Array(out_scenes);

    // music bed: a path, {file, gainDb, duckDb}, or {gen: "house"|"lofi"} for a handcrafted bed on
    // the beat grid; ducked under the voice
    let mut music_gen: Option<String> = None;
    if !spec["music"].is_null() {
        let mut m = if let Some(p) = spec["music"].as_str() {
            json!({ "file": p })
        } else {
            spec["music"].clone()
        };
        if let Some(g) = m["gen"].as_str() {
            if crate::synth::Style::parse(g).is_none() {
                return Err(format!("unknown music style `{g}` (house, lofi)").into());
            }
            if bpm.is_none() {
                return Err(
                    "`music.gen` needs a top-level `bpm` so the bed shares the beat grid".into(),
                );
            }
            music_gen = Some(g.to_string());
            m["file"] = json!(format!("assets/music/{name}.wav"));
        }
        let f = m["file"]
            .as_str()
            .ok_or("`music` needs a `file` (or `gen`)")?
            .to_string();
        let generated_pending =
            music_gen.is_some() && !require_generated && !root.join(&f).is_file();
        if generated_pending {
            m["file"] = Value::Null;
        } else {
            m["file"] = json!(resolve_asset(root, &f)?.0);
        }
        m["gainDb"] = json!(num(&m, "gainDb", -12.0, -60.0, 12.0));
        m["duckDb"] = json!(num(&m, "duckDb", -9.0, -40.0, 0.0));
        spec["music"] = m;
    }
    // sound effects on cuts and hits: on by default with a generated bed, or `"sfx": true`
    let want_sfx = spec["sfx"].as_bool().unwrap_or(music_gen.is_some());
    if want_sfx && bpm.is_some() {
        let rel = format!("assets/music/{name}-sfx.wav");
        if root.join(&rel).is_file() || require_generated {
            spec["sfx"] = json!({ "file": resolve_asset(root, &rel)?.0, "gainDb": num(&spec["sfx"], "gainDb", -8.0, -40.0, 6.0) });
        } else {
            spec["sfx"] = Value::Null;
        }
    } else {
        spec["sfx"] = Value::Null;
    }
    spec["subtitles"] = json!({
        "style": style,
        "enabled": sub_cfg["enabled"].as_bool().unwrap_or(true),
        "y": sub_cfg["y"].clone(),
        "size": sub_cfg["size"].clone(),
        "caps": sub_cfg["caps"].clone(),
    });
    spec["cues"] = Value::Array(cues.iter().map(Cue::to_json).collect());
    spec["beat"] = json!(beat);
    spec["voiceWindows"] = json!(windows.iter().map(|(a, b)| [a, b]).collect::<Vec<_>>());

    let known = [
        "name",
        "size",
        "aspect",
        "fps",
        "style",
        "theme",
        "bg",
        "scenes",
        "music",
        "duration",
        "watermark",
        "about",
        "bitrate",
        "voice",
        "subtitles",
        "cues",
        "voiceWindows",
        "bpm",
        "snap",
        "sfx",
        "beat",
    ];
    if let Some(m) = spec.as_object() {
        for k in m.keys().filter(|k| !known.contains(&k.as_str())) {
            warnings.push(format!("unknown top-level key `{k}` is ignored"));
        }
    }

    let js = |v: &Value| {
        serde_json::to_string(v)
            .unwrap_or_default()
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029")
    };
    let jsx = format!(
        "var SPEC = {};\nvar THEME = {};\nvar ROOT = \"__ROOT__\";\n{}",
        js(&spec),
        js(&theme),
        assets::COMPOSITOR_JSX
    );
    let plan = js(&json!({ "spec": spec, "theme": theme }));
    Ok(Built {
        name,
        jsx,
        plan,
        scene_count: scenes.len(),
        total,
        frame_times,
        warnings,
        cues,
        bpm,
        sfx: sfx_events,
        music_gen,
    })
}

/// Starter spec for `rsepuh new`: hook, chips, caption, call to action.
pub fn starter(name: &str, style: &str, aspect: &str, hook: &str) -> Value {
    let words: Vec<&str> = hook.split_whitespace().collect();
    json!({
        "name": name,
        "about": "Edit me. Every scene is plain JSON; see `rsepuh styles` for scene types.",
        "aspect": aspect,
        "fps": 30,
        "style": style,
        "scenes": [
            { "type": "hook", "dur": 2.4, "words": words, "sub": "your one-line promise" },
            { "type": "chips", "dur": 2.6, "title": "Three reasons", "items": ["Fast", "Honest", "Yours"] },
            { "type": "caption", "dur": 2.6, "text": "Say the point in one breath and let the words land" },
            { "type": "cta", "dur": 2.4, "title": name, "lines": ["your tagline"], "button": "Try it", "url": "example.com" }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rsepuh-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_spec(dir: &Path, v: Value) -> PathBuf {
        fs::create_dir_all(dir.join("scenes")).unwrap();
        let p = dir.join("scenes/x.json");
        fs::write(&p, v.to_string()).unwrap();
        p
    }

    #[test]
    fn starter_builds_and_totals_scene_durations() {
        let d = tmp("starter");
        let p = write_spec(&d, starter("demo", "punch", "9:16", "HELLO WORLD"));
        let b = build(&d, &p).unwrap();
        assert_eq!(b.name, "demo");
        assert!((b.total - 10.0).abs() < 1e-9);
        assert_eq!(b.frame_times.len(), 4);
        assert!(b.jsx.contains("var SPEC = ") && b.jsx.contains("lottieItem"));
        assert!(b.jsx.contains("\"size\":[1080,1920]"));
        assert!(b.plan.starts_with("{\"spec\":") && b.plan.contains("\"theme\":"));
    }

    #[test]
    fn explicit_at_overlaps_scenes_and_extends_total() {
        let d = tmp("at");
        let spec = json!({"scenes":[
            {"type":"title","text":"a","dur":3},
            {"type":"stat","value":5,"dur":2,"at":2.5}
        ]});
        let b = build(&d, &write_spec(&d, spec)).unwrap();
        assert!((b.total - 4.5).abs() < 1e-9);
    }

    #[test]
    fn rejects_bad_scenes_with_actionable_messages() {
        let d = tmp("bad");
        let cases = [
            (json!({"scenes":[]}), "non-empty"),
            (
                json!({"scenes":[{"type":"zoom","dur":1}]}),
                "unknown type `zoom`",
            ),
            (
                json!({"scenes":[{"type":"hook","dur":1}]}),
                "needs `words` or `text`",
            ),
            (
                json!({"scenes":[{"type":"hook","text":"a"}]}),
                "needs `dur`",
            ),
            (
                json!({"size":[1081,1920],"scenes":[{"type":"hook","text":"a","dur":1}]}),
                "even",
            ),
            (
                json!({"aspect":"2:1","scenes":[{"type":"hook","text":"a","dur":1}]}),
                "unknown aspect",
            ),
            (
                json!({"style":"nope","scenes":[{"type":"hook","text":"a","dur":1}]}),
                "unknown style `nope`",
            ),
        ];
        for (spec, want) in cases {
            let err = build(&d, &write_spec(&d, spec))
                .err()
                .expect("must fail")
                .to_string();
            assert!(err.contains(want), "{err} should contain {want}");
        }
    }

    #[test]
    fn assets_must_stay_inside_the_project_and_exist() {
        let d = tmp("assets");
        for src in ["/etc/passwd", "../x.png", "assets/missing.png"] {
            let spec = json!({"scenes":[{"type":"media","src":src,"dur":1}]});
            assert!(build(&d, &write_spec(&d, spec)).is_err(), "{src}");
        }
    }

    #[test]
    fn image_sequences_resolve_to_the_first_frame() {
        let d = tmp("seq");
        fs::create_dir_all(d.join("assets/3d/seal")).unwrap();
        for n in [3, 2, 7] {
            fs::write(d.join(format!("assets/3d/seal/seal_{n:04}.png")), b"x").unwrap();
        }
        for pat in [
            "assets/3d/seal/seal_%04d.png",
            "assets/3d/seal/seal_####.png",
        ] {
            let spec = json!({"scenes":[{"type":"media","overlay":pat,"dur":1}]});
            let b = build(&d, &write_spec(&d, spec)).unwrap();
            assert!(b.jsx.contains("assets/3d/seal/seal_0002.png"), "{pat}");
            assert!(b.jsx.contains("\"overlaySeq\":true"));
        }
    }

    #[test]
    fn custom_style_file_overrides_only_what_it_names() {
        let d = tmp("style");
        fs::create_dir_all(d.join("styles")).unwrap();
        fs::write(d.join("styles/mine.json"), r##"{"accent":"#00ff00"}"##).unwrap();
        let spec = json!({"style":"mine","theme":{"pop":1.5},"scenes":[{"type":"title","text":"a","dur":1}]});
        let b = build(&d, &write_spec(&d, spec)).unwrap();
        assert!(b.jsx.contains("\"accent\":\"#00ff00\""));
        assert!(b.jsx.contains("\"pop\":1.5"));
        assert!(b.jsx.contains("\"fg\":\"#ffffff\""), "keeps punch defaults");
    }

    #[test]
    fn custom_scenes_need_an_existing_python_writer_or_lottie_file() {
        let d = tmp("custom");
        fs::create_dir_all(d.join("lottie/scenes")).unwrap();
        fs::write(
            d.join("lottie/scenes/spin.py"),
            "def build(doc, sc, ctx): pass\n",
        )
        .unwrap();
        let ok = json!({"scenes":[{"type":"custom","file":"lottie/scenes/spin.py","dur":1}]});
        let b = build(&d, &write_spec(&d, ok)).unwrap();
        assert_eq!(b.scene_count, 1);
        assert!(
            b.plan.contains("lottie/scenes/spin.py"),
            "the writer reads the file name from the plan"
        );
        let missing = json!({"scenes":[{"type":"custom","file":"lottie/scenes/nope.py","dur":1}]});
        assert!(build(&d, &write_spec(&d, missing))
            .err()
            .unwrap()
            .to_string()
            .contains("does not exist"));
        let wrong = json!({"scenes":[{"type":"lottie","file":"assets/x.png","dur":1}]});
        assert!(build(&d, &write_spec(&d, wrong))
            .err()
            .unwrap()
            .to_string()
            .contains(".json (Lottie)"));
    }

    fn voiced_root(tag: &str, line: &str) -> (PathBuf, Value) {
        let d = tmp(tag);
        let spec = json!({
            "name": "v",
            "voice": {"engine": "edge", "lead": 0.1, "tail": 0.3},
            "scenes": [
                {"type": "title", "text": "a", "dur": 1.0, "voice": line},
                {"type": "stat", "value": 5, "dur": 2.0, "say": "five whole things here"}
            ]
        });
        let c = voice::cfg(&spec).unwrap();
        fs::create_dir_all(d.join("assets/voice/v")).unwrap();
        fs::write(d.join("assets/voice/v/s00.mp3"), b"x").unwrap();
        let entry = voice::Entry {
            index: 0,
            text: line.into(),
            hash: voice::hash(&c, line),
            file: "assets/voice/v/s00.mp3".into(),
            duration: 2.5,
            words: subs::distribute(line, 0.05, 2.4),
            heard: None,
        };
        voice::save(&d, "v", &c, &[entry]).unwrap();
        (d, spec)
    }

    #[test]
    fn voice_sets_scene_length_audio_placement_and_cues() {
        let (d, spec) = voiced_root("voice", "hello there wonderful world");
        let b = build(&d, &write_spec(&d, spec)).unwrap();
        // scene 1: max(1.0, lead 0.1 + 2.5 + tail 0.3) = 2.9; scene 2 starts there
        assert!((b.total - 4.9).abs() < 1e-9, "{}", b.total);
        assert!(b.jsx.contains("\"voiceFile\":\"assets/voice/v/s00.mp3\""));
        assert!(b.jsx.contains("\"voiceAt\":0.1"));
        // 4 voiced words (4 max per cue) + 4 `say` words
        assert_eq!(b.cues.len(), 2);
        assert!(
            (b.cues[0].words[0].s - 0.15).abs() < 1e-9,
            "word times are absolute: lead + engine time"
        );
        let say = &b.cues[1];
        assert!(
            say.s >= 2.9 + 0.15 - 1e-9 && say.e <= 4.9,
            "say cues stay inside their scene"
        );
        assert!(b.jsx.contains("\"voiceWindows\":[[0.1,2.6]]"));
    }

    #[test]
    fn stale_or_missing_voice_clips_stop_the_build_with_the_fix() {
        let (d, mut spec) = voiced_root("stale", "hello there");
        spec["scenes"][0]["voice"] = json!("hello there, changed");
        let err = build(&d, &write_spec(&d, spec.clone()))
            .err()
            .unwrap()
            .to_string();
        assert!(
            err.contains("out of date") && err.contains("rsepuh voice"),
            "{err}"
        );
        let empty = tmp("noclip");
        let err = build(&empty, &write_spec(&empty, spec))
            .err()
            .unwrap()
            .to_string();
        assert!(err.contains("no clip yet"), "{err}");
    }

    #[test]
    fn subtitle_file_and_style_validation() {
        let d = tmp("subs");
        fs::create_dir_all(d.join("assets/subs")).unwrap();
        fs::write(
            d.join("assets/subs/x.srt"),
            "1\n00:00:01,000 --> 00:00:02,000\nhalo dunia ini\n",
        )
        .unwrap();
        let ok = json!({"subtitles": {"file": "assets/subs/x.srt", "offset": 0.5, "style": "marker"},
                        "scenes": [{"type": "title", "text": "a", "dur": 3}]});
        let b = build(&d, &write_spec(&d, ok)).unwrap();
        assert_eq!(b.cues.len(), 1);
        assert!((b.cues[0].s - 1.5).abs() < 1e-9);
        assert!(b.jsx.contains("\"style\":\"marker\""));
        let bad = json!({"subtitles": {"style": "sparkle"}, "scenes": [{"type": "title", "text": "a", "dur": 1}]});
        assert!(build(&d, &write_spec(&d, bad))
            .err()
            .unwrap()
            .to_string()
            .contains("unknown subtitle style"));
        let hidden = json!({"scenes": [{"type": "title", "text": "a", "dur": 3, "say": "nope", "subtitle": false}]});
        assert!(build(&d, &write_spec(&d, hidden)).unwrap().cues.is_empty());
    }

    #[test]
    fn music_gets_defaults_and_must_exist() {
        let d = tmp("music");
        let spec = json!({"music": "assets/music/bed.mp3", "scenes": [{"type": "title", "text": "a", "dur": 1}]});
        assert!(build(&d, &write_spec(&d, spec.clone())).is_err());
        fs::create_dir_all(d.join("assets/music")).unwrap();
        fs::write(d.join("assets/music/bed.mp3"), b"x").unwrap();
        let b = build(&d, &write_spec(&d, spec)).unwrap();
        assert!(
            b.jsx.contains("\"gainDb\":-12.0") && b.jsx.contains("\"duckDb\":-9.0"),
            "{}",
            &b.jsx[..300]
        );
    }

    #[test]
    fn on_screen_moments_pin_to_spoken_words() {
        let (d, mut spec) = voiced_root("onword", "React owns the screen and the outside world");
        spec["bpm"] = json!(120);
        spec["scenes"][0] = json!({
            "type": "custom", "file": "lottie/scenes/x.py", "dur": 1.0,
            "voice": "React owns the screen and the outside world",
            "onWord": {"bottomBeat": "outside", "pauseT": "owns"},
            "onWords": {"stepBeats": ["react", "screen", "world"]},
            "focus": [{"line": 0, "atWord": "screen", "untilWord": "and"}, {"line": 1, "atWord": "outside"}]
        });
        fs::create_dir_all(d.join("lottie/scenes")).unwrap();
        fs::write(d.join("lottie/scenes/x.py"), "").unwrap();
        // the clip was saved for the old text: refresh its entry for this one
        let c = voice::cfg(&spec).unwrap();
        let line = "React owns the screen and the outside world";
        let e = voice::Entry {
            index: 0,
            text: line.into(),
            hash: voice::hash(&c, line),
            file: "assets/voice/v/s00.mp3".into(),
            duration: 2.5,
            words: subs::distribute(line, 0.0, 2.4),
            heard: None,
        };
        voice::save(&d, "v", &c, std::slice::from_ref(&e)).unwrap();
        let b = build(&d, &write_spec(&d, spec.clone())).unwrap();
        let w = &voice::load(&d, "v")[0].words; // as saved: times are rounded to the millisecond
        let beats = |t: f64| ((t + 0.1) / 0.5 * 1000.0).round() / 1000.0; // the voice starts 0.1 s (lead) into the scene
        assert!(
            b.jsx.contains(&format!("\"bottomBeat\":{}", beats(w[6].s))),
            "outside word start in beats; want {} got {:?}",
            beats(w[6].s),
            b.jsx
                .match_indices("\"bottomBeat\"")
                .map(|(i, _)| b.jsx[i..i + 30].to_string())
                .collect::<Vec<_>>()
        );
        assert!(b.jsx.contains("\"stepBeats\":["));
        assert!(b.jsx.contains("\"pauseT\":"), "T fields are seconds");
        assert!(
            b.jsx.contains("\"untilBeat\":"),
            "focus gets until from untilWord or +4 beats"
        );
        spec["scenes"][0]["onWord"] = json!({"bottomBeat": "nonexistent"});
        let err = build(&d, &write_spec(&d, spec)).err().unwrap().to_string();
        assert!(
            err.contains("`nonexistent` is not spoken") && err.contains("voice says"),
            "{err}"
        );
    }

    #[test]
    fn slug_and_aspects() {
        assert_eq!(slug("My Video!"), "my-video");
        assert_eq!(slug("!!!"), "video");
        assert_eq!(aspect_size("16:9"), Some((1920, 1080)));
    }
}
