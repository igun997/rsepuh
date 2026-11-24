//! Command implementations. Every command prints for humans and returns a process exit code;
//! the MCP server runs the same binary and returns what it printed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::assets;
use crate::audio;
use crate::gemini;
use crate::remote::{self, Remote, RenderOpts};
use crate::sheet;
use crate::spec;
use crate::subs;
use crate::util::sh_quote;
use crate::verify;
use crate::voice;
use crate::Result;

pub const HELP: &str = "\
rsepuh: Edit layaknya sepuh. UGC-style motion videos with EffectCraft + Blender (+ optional Gemini
stills and voice), rendered on a remote sandbox (Daytona, instabox, any SSH host) through rcargo.

USAGE: rsepuh [-i INSTANCE] <command> [args]

  init [--style S]         make this folder a project: Cargo stub, scenes/, skills, MCP, Blender scripts
  doctor [--blender]       check rcargo, the sandbox, EffectCraft, voice engines and keys
  setup [--blender]        install EffectCraft (and Blender) on the sandbox now
  styles                   list motion styles, scene types, subtitle styles, voice engines
  new NAME [--style S] [--aspect 9:16] [--text \"HOOK WORDS\"] [--force]
                           write scenes/NAME.json from a starter
  voice SPEC [--force] [--env FILE]
                           speak every scene's `voice` line (edge = free, gemini) and time the words
  voices [--engine edge|gemini] [--lang id]
                           list voices
  music SPEC [--style house|lofi]
                           handcrafted music bed + sound effects on the spec's beat grid (needs bpm in the spec)
  render SPEC [--draft] [--bitrate KBPS] [--no-pull] [--env FILE]
                           build, render on the sandbox, pull MP4 + contact sheet + subtitles
  frame SPEC --at SECONDS [--scale 0.5]
                           render one preview frame
  subs SPEC [--format srt|vtt|both]
                           write SRT/VTT for platforms that take caption files (exports/NAME/)
  build SPEC [--out FILE] [--plan]
                           print/save the compositor script, or with --plan the motion writer input (debug)
  blender SCRIPT [--out NAME] [ARGS]
                           run a Blender script (text3d, medal, or your own) via rblender,
                           pull the PNG sequence into assets/3d/NAME
  gen image PROMPT [--out PATH] [--aspect 9:16] [--ref FILE]... [--model M] [--env FILE]
                           Gemini still image (never video)
  ec -- ARGS               raw effectcraft-cli on the sandbox
  verify PATH              prove a render is real (MP4 parsed, PNG decoded)
  check FILE [--platform tiktok|reels|shorts|all]
                           test an MP4 against platform upload limits
  pv [--engine edge|gemini]
                           build the 10 second rsepuh PV end to end
  skills [--global]        (re)install the agent skills
  mcp                      MCP server over stdio
  down                     pause the sandbox (it wakes on next use)

SPEC is scenes/NAME.json or just NAME. Choose a machine with -i NAME or RCARGO_INSTANCE.
Environment: EFFECTCRAFT_VERSION, GEMINI_API_KEY or GEMINI_KEY (only for gen/voice with gemini).";

struct Args(Vec<String>);

impl Args {
    fn flag(&mut self, f: &str) -> bool {
        let n = self.0.len();
        self.0.retain(|a| a != f);
        self.0.len() != n
    }
    fn opt(&mut self, f: &str) -> Result<Option<String>> {
        if let Some(i) = self.0.iter().position(|a| a == f) {
            if i + 1 >= self.0.len() {
                return Err(format!("{f} needs a value").into());
            }
            let v = self.0.remove(i + 1);
            self.0.remove(i);
            return Ok(Some(v));
        }
        Ok(None)
    }
    fn opts(&mut self, f: &str) -> Result<Vec<String>> {
        let mut out = Vec::new();
        while let Some(v) = self.opt(f)? {
            out.push(v);
        }
        Ok(out)
    }
    fn first(&mut self) -> Option<String> {
        (!self.0.is_empty()).then(|| self.0.remove(0))
    }
}

fn status(st: std::process::ExitStatus) -> u8 {
    if st.success() {
        0
    } else {
        st.code().unwrap_or(1).clamp(1, 255) as u8
    }
}

fn human(b: u64) -> String {
    if b >= 1 << 20 {
        format!("{:.1} MiB", b as f64 / (1 << 20) as f64)
    } else {
        format!("{:.1} KiB", b as f64 / 1024.0)
    }
}

pub fn run(mut argv: Vec<String>) -> Result<u8> {
    let mut instance = std::env::var("RCARGO_INSTANCE")
        .ok()
        .filter(|s| !s.is_empty());
    if matches!(argv.first().map(String::as_str), Some("-i" | "--instance")) {
        if argv.len() < 2 {
            return Err("-i needs an instance name".into());
        }
        instance = Some(argv[1].clone());
        argv.drain(..2);
    }
    if matches!(
        argv.first().map(String::as_str),
        Some("--version" | "-V" | "version")
    ) {
        println!("rsepuh {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    if argv.is_empty() || matches!(argv[0].as_str(), "help" | "-h" | "--help") {
        println!("{HELP}");
        return Ok(0);
    }
    let cmd = argv.remove(0);
    let mut a = Args(argv);
    let remote = Remote {
        instance,
        version: std::env::var("EFFECTCRAFT_VERSION")
            .unwrap_or_else(|_| assets::EFFECTCRAFT_VERSION.into()),
    };
    let root = std::env::current_dir()?;
    match cmd.as_str() {
        "init" => init(&root, &mut a),
        "doctor" => doctor(&remote, a.flag("--blender")),
        "setup" => {
            remote.ensure()?;
            println!(
                "rsepuh: EffectCraft {} ready on the sandbox",
                remote.version
            );
            if a.flag("--blender") {
                return Ok(status(
                    remote
                        .rblender(&["setup"])
                        .status()
                        .map_err(Remote::spawn_err)?,
                ));
            }
            Ok(0)
        }
        "styles" => styles(),
        "new" => new_spec(&root, &mut a),
        "voice" => voice_cmd(&root, &mut a),
        "voices" => voices_cmd(&mut a),
        "music" => music_cmd(&root, &mut a),
        "render" => render(&root, &remote, &mut a),
        "frame" => frame(&root, &remote, &mut a),
        "subs" => subs_cmd(&root, &mut a),
        "build" => build_cmd(&root, &mut a),
        "blender" => blender(&root, &remote, &mut a),
        "gen" => gen(&root, &mut a),
        "ec" => {
            let raw: Vec<String> = a.0.into_iter().skip_while(|x| x == "--").collect();
            remote.ensure()?;
            let args: String = raw.iter().map(|x| format!(" {}", sh_quote(x))).collect();
            Ok(status(remote.shell(
                &format!("exec \"$HOME/effectcraft/bin/effectcraft-cli\"{args}"),
                false,
                false,
            )?))
        }
        "verify" => verify_cmd(&mut a),
        "check" => check_cmd(&mut a),
        "pv" => pv(&root, &remote, &mut a),
        "skills" => {
            let global = a.flag("--global");
            install_skills(&root, global)?;
            Ok(0)
        }
        "mcp" => {
            crate::mcp::serve(remote.instance.clone())?;
            Ok(0)
        }
        "down" => Ok(status(
            Command::new("rcargo")
                .args(["remote", "down"])
                .status()
                .map_err(Remote::spawn_err)?,
        )),
        other => Err(format!("unknown command `{other}` (see `rsepuh help`)").into()),
    }
}

// ------------------------------------------------------------------ project

fn spec_path(root: &Path, arg: &str) -> Result<PathBuf> {
    let p = Path::new(arg);
    let cands = [
        p.to_path_buf(),
        root.join("scenes").join(format!("{arg}.json")),
        root.join("scenes").join(arg),
    ];
    cands.into_iter().find(|c| c.is_file()).ok_or_else(|| {
        format!(
            "no spec `{arg}` (looked for it and scenes/{arg}.json). Create one: rsepuh new {arg}"
        )
        .into()
    })
}

/// The motion writer lives in the project (`lottie/`) so you can read and change it; copy any file
/// that is missing, never overwrite one you edited.
fn ensure_lottie(root: &Path) -> Result<usize> {
    let mut n = 0;
    for (rel, body) in assets::LOTTIE_FILES {
        n += usize::from(write_if_absent(&root.join(rel), body)?);
    }
    Ok(n)
}

fn write_if_absent(path: &Path, body: &str) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    fs::write(path, body)?;
    Ok(true)
}

fn install_skills(root: &Path, global: bool) -> Result<()> {
    let bases: Vec<PathBuf> = if global {
        let home = std::env::var("HOME").map_err(|_| "HOME is not set")?;
        vec![
            PathBuf::from(&home).join(".claude/skills"),
            PathBuf::from(&home).join(".agents/skills"),
        ]
    } else {
        vec![root.join(".claude/skills"), root.join(".agents/skills")]
    };
    for base in &bases {
        for (name, body) in assets::SKILLS {
            let d = base.join(name);
            fs::create_dir_all(&d)?;
            fs::write(d.join("SKILL.md"), body)?;
        }
        println!(
            "rsepuh: installed {} skills in {}",
            assets::SKILLS.len(),
            base.display()
        );
    }
    Ok(())
}

fn add_mcp(root: &Path) -> Result<()> {
    let p = root.join(".mcp.json");
    let mut v: Value = fs::read_to_string(&p)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    if !v["mcpServers"].is_object() {
        v["mcpServers"] = json!({});
    }
    if v["mcpServers"]["rsepuh"].is_null() {
        v["mcpServers"]["rsepuh"] = json!({"command": "rsepuh", "args": ["mcp"]});
        fs::write(&p, serde_json::to_string_pretty(&v)? + "\n")?;
        println!("rsepuh: registered the rsepuh MCP server in .mcp.json");
    }
    Ok(())
}

fn init(root: &Path, a: &mut Args) -> Result<u8> {
    let style = a.opt("--style")?.unwrap_or_else(|| "punch".into());
    if spec::builtin_theme(&style).is_none() {
        return Err(format!("unknown style `{style}` (see `rsepuh styles`)").into());
    }
    if !root.ancestors().any(|d| d.join("Cargo.toml").exists()) {
        let name = spec::slug(
            &root
                .file_name()
                .map_or("video".into(), |n| n.to_string_lossy().into_owned()),
        );
        // rcargo anchors project sync on a cargo project; this stub makes any folder one.
        fs::write(
            "Cargo.toml",
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n"),
        )?;
        fs::create_dir_all("src")?;
        fs::write("src/lib.rs", "")?;
        println!("rsepuh: created Cargo.toml stub ({name})");
    }
    let gi = fs::read_to_string(".gitignore").unwrap_or_default();
    let mut add = String::new();
    for line in ["/renders", "/target", ".env"] {
        if !gi.lines().any(|l| l.trim() == line) {
            add += &format!("{line}\n");
        }
    }
    if !add.is_empty() {
        let sep = if gi.is_empty() || gi.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        fs::write(".gitignore", format!("{gi}{sep}{add}"))?;
    }
    for d in [
        "scenes",
        "styles",
        "blender",
        "assets/gen",
        "assets/3d",
        "assets/voice",
        "assets/music",
    ] {
        fs::create_dir_all(d)?;
    }
    for (name, body) in assets::BLENDER_SCRIPTS {
        write_if_absent(&root.join("blender").join(format!("{name}.py")), body)?;
    }
    if ensure_lottie(root)? > 0 {
        println!("rsepuh: wrote the motion writer to lottie/ (Python; edit it or add scenes in lottie/scenes/)");
    }
    let starter = spec::starter("hello", &style, "9:16", "HELLO WORLD");
    if write_if_absent(
        &root.join("scenes/hello.json"),
        &(serde_json::to_string_pretty(&starter)? + "\n"),
    )? {
        println!("rsepuh: wrote scenes/hello.json (try: rsepuh render hello --draft)");
    }
    install_skills(root, false)?;
    add_mcp(root)?;
    Ok(status(
        Command::new("rcargo")
            .arg("init")
            .status()
            .map_err(Remote::spawn_err)?,
    ))
}

fn styles() -> Result<u8> {
    println!("Motion styles (spec: \"style\"; add your own as styles/NAME.json, it only needs the keys it changes):");
    for (name, body) in assets::THEMES {
        let v: Value = serde_json::from_str(body)?;
        println!("  {name:<8} {}", v["about"].as_str().unwrap_or(""));
    }
    println!(
        "\nScene types (each takes `dur` seconds, or a `voice` line to take its length from). Their motion is Lottie written by lottie/writer.py on the sandbox:"
    );
    for (t, f) in [
        (
            "hook",
            "words | text, sub?, beat?, colors?: fast one-word-at-a-time opener",
        ),
        (
            "title",
            "lines | text, sub?: big headline, last line in accent",
        ),
        (
            "media",
            "src? (image/video/sequence), overlay? (alpha PNG sequence), caption?, dim?, zoom?",
        ),
        (
            "chips",
            "items[], title?, sub?: stickers that pop in with stagger",
        ),
        (
            "terminal",
            "lines[] (lines starting with $ are typed), cps?",
        ),
        (
            "stat",
            "value, from?, prefix?, suffix?, label?: counting number",
        ),
        (
            "caption",
            "text, src?, times?, y?, size?: karaoke words over a scene",
        ),
        ("cta", "title, lines?, button?, url?: end card"),
        (
            "code",
            "code[], file_name?, focus[]: typed code window, glide bar, callout cards",
        ),
        (
            "lottie",
            "file: a Lottie you authored (text-to-lottie skill), fit?",
        ),
        (
            "custom",
            "file: your lottie/scenes/NAME.py (build(doc, sc, ctx)) or a Lottie .json",
        ),
    ] {
        println!("  {t:<9} {f}");
    }
    println!("\nPer scene: at? (start), transition? (cut|push|flash|accent|wipe|dip), caps? (false keeps your casing),");
    println!("           voice? (spoken line), say? (on-screen subtitle without sound), subtitle? (false hides it)");
    println!("\nSubtitle styles (spec: \"subtitles\": {{\"style\": ...}}; cues come from voice lines, `say`, or an SRT/VTT `file`):");
    for (n, d) in [
        (
            "karaoke",
            "phrase on screen, the spoken word pops in the accent colour",
        ),
        ("pop", "one big word at a time, centre screen"),
        (
            "box",
            "phrase on a dark rounded box, spoken word highlighted",
        ),
        (
            "outline",
            "classic white text with a black outline, no highlight",
        ),
        ("typewriter", "phrase types out in the mono font"),
        ("marker", "highlighter bar slides onto the spoken word"),
        ("lower", "small documentary strip across the bottom"),
        ("glow", "karaoke with a glowing spoken word"),
    ] {
        println!("  {n:<11} {d}");
    }
    println!("\nVoice (spec: \"voice\": {{\"engine\": ...}}):");
    println!("  edge     free, Microsoft Edge voices through edge-tts, real word timings (needs uv or a current edge-tts)");
    println!(
        "  gemini   Gemini TTS ({}), one request per scene, word times estimated from the speech",
        voice::GEMINI_TTS_MODEL
    );
    Ok(0)
}

fn new_spec(root: &Path, a: &mut Args) -> Result<u8> {
    let style = a.opt("--style")?.unwrap_or_else(|| "punch".into());
    let aspect = a.opt("--aspect")?.unwrap_or_else(|| "9:16".into());
    let hook = a.opt("--text")?.unwrap_or_else(|| "MAKE IT MOVE".into());
    let force = a.flag("--force");
    let name = a
        .first()
        .ok_or("usage: rsepuh new NAME [--style S] [--aspect 9:16] [--text \"HOOK WORDS\"]")?;
    if spec::builtin_theme(&style).is_none()
        && !root
            .join("styles")
            .join(format!("{}.json", spec::slug(&style)))
            .is_file()
    {
        return Err(format!("unknown style `{style}` (see `rsepuh styles`)").into());
    }
    if spec::aspect_size(&aspect).is_none() {
        return Err(format!("unknown aspect `{aspect}` (9:16, 16:9, 1:1, 4:5, 3:4)").into());
    }
    let slug = spec::slug(&name);
    let path = root.join("scenes").join(format!("{slug}.json"));
    if path.exists() && !force {
        return Err(format!("{} exists (use --force to overwrite)", path.display()).into());
    }
    fs::create_dir_all(root.join("scenes"))?;
    let v = spec::starter(&slug, &style, &aspect, &hook);
    fs::write(&path, serde_json::to_string_pretty(&v)? + "\n")?;
    println!("rsepuh: wrote scenes/{slug}.json  (render it: rsepuh render {slug} --draft)");
    Ok(0)
}

// ------------------------------------------------------------------ voice, subtitles

fn load_spec_value(path: &Path) -> Result<Value> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)
        .map_err(|e| format!("{} is not valid JSON: {e}", path.display()))?)
}

fn voice_cmd(root: &Path, a: &mut Args) -> Result<u8> {
    let force = a.flag("--force");
    let env = a.opt("--env")?.map(PathBuf::from);
    let p = spec_path(
        root,
        &a.first()
            .ok_or("usage: rsepuh voice SPEC [--force] [--env FILE]")?,
    )?;
    let v = load_spec_value(&p)?;
    let name = spec::slug(
        v["name"].as_str().unwrap_or(
            &p.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
    );
    for line in voice::synth(root, &name, &v, force, env.as_deref())? {
        println!("rsepuh: voice {line}");
    }
    println!(
        "rsepuh: timeline assets/voice/{name}/timeline.json (render now: rsepuh render {name})"
    );
    Ok(0)
}

fn voices_cmd(a: &mut Args) -> Result<u8> {
    let engine = a.opt("--engine")?.unwrap_or_else(|| "edge".into());
    let lang = a.opt("--lang")?.unwrap_or_else(|| "en".into());
    for v in voice::list_voices(&engine, &lang)? {
        println!("{v}");
    }
    Ok(0)
}

fn music_cmd(root: &Path, a: &mut Args) -> Result<u8> {
    let style_arg = a.opt("--style")?;
    let p = spec_path(
        root,
        &a.first()
            .ok_or("usage: rsepuh music SPEC [--style house|lofi]")?,
    )?;
    let b = spec::build_opts(root, &p, false)?;
    let bpm = b.bpm.ok_or(
        "add a top-level \"bpm\" to the spec: the music, cuts and pops share its beat grid",
    )?;
    let name = b
        .music_gen
        .clone()
        .or(style_arg)
        .unwrap_or_else(|| "house".into());
    let style = crate::synth::Style::parse(&name)
        .ok_or_else(|| format!("unknown music style `{name}` (house, lofi)"))?;
    let dir = root.join("assets/music");
    fs::create_dir_all(&dir)?;
    let secs = b.total;
    let music_path = dir.join(format!("{}.wav", b.name));
    fs::write(
        &music_path,
        audio::wav_from_pcm16(
            &crate::synth::music(bpm, secs, style),
            crate::synth::RATE,
            1,
        ),
    )?;
    let sfx_path = dir.join(format!("{}-sfx.wav", b.name));
    fs::write(
        &sfx_path,
        audio::wav_from_pcm16(&crate::synth::sfx(&b.sfx, secs), crate::synth::RATE, 1),
    )?;
    let beat = 60.0 / bpm;
    println!(
        "rsepuh: music {} at {bpm} BPM, {:.2}s = {} beats ({} bars), beat 1 at 0.00s -> {}",
        name,
        secs,
        (secs / beat).round() as u32,
        (secs / beat / 4.0).round() as u32,
        music_path.display()
    );
    println!(
        "rsepuh: {} sound effects on cuts and hits -> {}",
        b.sfx.len(),
        sfx_path.display()
    );
    if b.music_gen.is_none() {
        println!("rsepuh: to use it, add  \"music\": {{ \"gen\": \"{name}\" }}  to the spec");
    }
    Ok(0)
}

fn subs_cmd(root: &Path, a: &mut Args) -> Result<u8> {
    let format = a.opt("--format")?.unwrap_or_else(|| "both".into());
    let p = spec_path(
        root,
        &a.first()
            .ok_or("usage: rsepuh subs SPEC [--format srt|vtt|both]")?,
    )?;
    let built = spec::build(root, &p)?;
    if built.cues.is_empty() {
        return Err(
            "the spec has no subtitle cues (add voice or say lines to scenes, or subtitles.file)"
                .into(),
        );
    }
    // A separate directory from the MP4 (exports/NAME, not renders/NAME):
    // a player that auto-loads a sidecar would show the captions twice over the burned-in ones.
    let dir = root.join("exports").join(&built.name);
    fs::create_dir_all(&dir)?;
    write_sidecars(&dir, &built.name, &built.cues, &format)?;
    println!("rsepuh: the video already has burned-in subtitles; upload these only where a platform asks for a caption file, and never put them beside the MP4");
    Ok(0)
}

fn write_sidecars(dir: &Path, name: &str, cues: &[subs::Cue], format: &str) -> Result<()> {
    if matches!(format, "srt" | "both") {
        fs::write(dir.join(format!("{name}.srt")), subs::to_srt(cues))?;
    }
    if matches!(format, "vtt" | "both") {
        fs::write(dir.join(format!("{name}.vtt")), subs::to_vtt(cues))?;
    }
    println!(
        "rsepuh: subtitles ({} cues) in {} ({format})",
        cues.len(),
        dir.display()
    );
    Ok(())
}

// ------------------------------------------------------------------ build and render

fn build_cmd(root: &Path, a: &mut Args) -> Result<u8> {
    let out = a.opt("--out")?;
    let plan = a.flag("--plan");
    let p = spec_path(
        root,
        &a.first()
            .ok_or("usage: rsepuh build SPEC [--out FILE] [--plan]")?,
    )?;
    let b = spec::build(root, &p)?;
    let body = if plan { &b.plan } else { &b.jsx };
    for w in &b.warnings {
        eprintln!("rsepuh: warning: {w}");
    }
    match out {
        Some(f) => {
            fs::write(&f, body)?;
            println!(
                "rsepuh: wrote {f} ({} scenes worth, {:.1}s)",
                b.frame_times.len(),
                b.total
            );
        }
        None => println!("{body}"),
    }
    Ok(0)
}

fn summary(b: &spec::Built) {
    println!(
        "rsepuh: {} scenes, {:.2}s, {} subtitle cues",
        b.frame_times.len(),
        b.total,
        b.cues.len()
    );
    for w in &b.warnings {
        eprintln!("rsepuh: warning: {w}");
    }
}

fn render(root: &Path, remote: &Remote, a: &mut Args) -> Result<u8> {
    let draft = a.flag("--draft");
    let no_pull = a.flag("--no-pull");
    let bitrate: Option<u32> = a
        .opt("--bitrate")?
        .map(|s| s.parse())
        .transpose()
        .map_err(|_| "--bitrate must be a number of kbps")?;
    let _ = a.opt("--env")?;
    let p = spec_path(
        root,
        &a.first()
            .ok_or("usage: rsepuh render SPEC [--draft] [--bitrate KBPS]")?,
    )?;
    let b = spec::build(root, &p)?;
    summary(&b);
    let spec_v = load_spec_value(&p)?;
    let kbps = bitrate
        .or_else(|| spec_v["bitrate"].as_u64().map(|v| v as u32))
        .unwrap_or(if draft { 3000 } else { 8000 });
    remote.ensure()?;
    ensure_lottie(root)?;
    let script = remote::render_script(&RenderOpts {
        name: &b.name,
        jsx: &b.jsx,
        plan: &b.plan,
        scenes: b.scene_count,
        frame_times: &b.frame_times,
        draft,
        bitrate_kbps: kbps,
    });
    eprintln!(
        "rsepuh: rendering{} on the sandbox",
        if draft {
            " (draft, half resolution)"
        } else {
            ""
        }
    );
    let st = remote.shell(&script, false, false)?;
    if !st.success() {
        eprintln!("rsepuh: the sandbox build or render failed ({st}); output above names the line");
        return Ok(status(st));
    }
    if no_pull {
        return Ok(0);
    }
    let dest = root.join("renders").join(&b.name);
    let _ = fs::remove_dir_all(&dest);
    let bytes = remote.pull(&format!("renders/{}", b.name), &dest)?;
    println!("\nrsepuh: pulled {} to renders/{}", human(bytes), b.name);
    let mp4 = dest.join(format!("{}.mp4", b.name));
    if let Ok(n) = sheet::make(
        &dest.join("frames"),
        &dest.join(format!("{}.sheet.png", b.name)),
        4,
    ) {
        println!("  {}.sheet.png        contact sheet of {n} scenes", b.name);
    }
    if !b.cues.is_empty() {
        println!("  subtitles are burned into the video; `rsepuh subs {}` writes SRT/VTT only if a platform needs a caption file", b.name);
    }
    match verify::file(&mp4) {
        Some(f) => {
            println!(
                "  [{}] {:<24} {}",
                if f.ok { " ok " } else { "FAIL" },
                f.file,
                f.detail
            );
            if !f.ok {
                return Ok(1);
            }
        }
        None => return Err("no MP4 was produced".into()),
    }
    print_platform_summary(&mp4);
    Ok(0)
}

fn print_platform_summary(mp4: &Path) {
    let Ok(bytes) = fs::read(mp4) else { return };
    let Ok(m) = verify::parse_mp4(&bytes) else {
        return;
    };
    for p in verify::PLATFORMS {
        let bad: Vec<String> = verify::check_platform(&m, p)
            .into_iter()
            .filter(|(ok, _)| !ok)
            .map(|(_, msg)| msg)
            .collect();
        if bad.is_empty() {
            println!("  {:<16} fits", p.name);
        } else {
            println!("  {:<16} {}", p.name, bad.join("; "));
        }
    }
}

fn frame(root: &Path, remote: &Remote, a: &mut Args) -> Result<u8> {
    let at: f64 = a
        .opt("--at")?
        .ok_or("usage: rsepuh frame SPEC --at SECONDS [--scale 0.5]")?
        .parse()
        .map_err(|_| "--at must be seconds")?;
    let scale: f64 = a
        .opt("--scale")?
        .map(|s| s.parse())
        .transpose()
        .map_err(|_| "--scale must be a number")?
        .unwrap_or(0.5);
    let p = spec_path(
        root,
        &a.first().ok_or("usage: rsepuh frame SPEC --at SECONDS")?,
    )?;
    let b = spec::build(root, &p)?;
    remote.ensure()?;
    ensure_lottie(root)?;
    let script = remote::frame_script(
        &RenderOpts {
            name: &b.name,
            jsx: &b.jsx,
            plan: &b.plan,
            scenes: b.scene_count,
            frame_times: &[],
            draft: false,
            bitrate_kbps: 0,
        },
        at,
        scale,
    );
    let st = remote.shell(&script, false, false)?;
    if !st.success() {
        return Ok(status(st));
    }
    let dest = root.join("renders").join(&b.name);
    remote.pull(&format!("renders/{}", b.name), &dest)?;
    let out = dest.join(format!("frame_{at:.2}.png"));
    fs::rename(dest.join("frame.png"), &out)?;
    println!("rsepuh: {}", out.display());
    Ok(0)
}

// ------------------------------------------------------------------ blender, gen

fn blender(root: &Path, remote: &Remote, a: &mut Args) -> Result<u8> {
    let out = a.opt("--out")?;
    let script = a.first().ok_or(
        "usage: rsepuh blender SCRIPT [--out NAME] [ARGS]  (text3d, medal, or blender/NAME.py)",
    )?;
    for (name, body) in assets::BLENDER_SCRIPTS {
        write_if_absent(&root.join("blender").join(format!("{name}.py")), body)?;
    }
    let rel = if script.ends_with(".py") {
        script.clone()
    } else {
        format!("blender/{script}.py")
    };
    if !root.join(&rel).is_file() {
        return Err(format!(
            "{rel} not found. Bundled: {}",
            assets::BLENDER_SCRIPTS
                .iter()
                .map(|(n, _)| *n)
                .filter(|n| *n != "stage")
                .collect::<Vec<_>>()
                .join(", ")
        )
        .into());
    }
    let bundled = assets::BLENDER_SCRIPTS
        .iter()
        .any(|(n, _)| rel == format!("blender/{n}.py"));
    let stem = Path::new(&rel)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = spec::slug(&out.clone().unwrap_or(stem));
    let mut args: Vec<String> = vec!["run".into(), rel.clone(), "--no-pull".into()];
    if bundled && !a.0.iter().any(|x| x == "--name") {
        args.push("--name".into());
        args.push(name.clone());
    }
    args.append(&mut a.0);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    eprintln!("rsepuh: running {rel} in Blender on the sandbox (rblender)");
    let st = remote.rblender(&refs).status().map_err(blender_spawn_err)?;
    if !st.success() {
        return Ok(status(st));
    }
    let tmp = root.join("assets/3d").join(format!(".pull-{name}"));
    let _ = fs::remove_dir_all(&tmp);
    let tmp_s = tmp.to_string_lossy().into_owned();
    let st = remote
        .rblender(&["pull", &tmp_s])
        .status()
        .map_err(blender_spawn_err)?;
    if !st.success() {
        return Ok(status(st));
    }
    let dest = root.join("assets/3d").join(&name);
    let _ = fs::remove_dir_all(&dest);
    let inner = tmp.join(&name);
    if inner.is_dir() {
        fs::rename(&inner, &dest)?;
        let _ = fs::rename(tmp.join("report.json"), dest.join("report.json"));
        let _ = fs::remove_dir_all(&tmp);
    } else {
        fs::rename(&tmp, &dest)?;
    }
    report_sequence(&dest, &name)
}

fn blender_spawn_err(e: std::io::Error) -> Box<dyn std::error::Error> {
    if e.kind() == std::io::ErrorKind::NotFound {
        "rblender is not installed. Install: curl -fsSL https://raw.githubusercontent.com/igun997/rblender/main/install.sh | sh".into()
    } else {
        e.into()
    }
}

fn report_sequence(dest: &Path, name: &str) -> Result<u8> {
    let mut frames: Vec<PathBuf> = fs::read_dir(dest)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    frames.sort();
    if frames.is_empty() {
        println!(
            "rsepuh: pulled {} (no PNG frames; use the files as footage)",
            dest.display()
        );
        return Ok(0);
    }
    let probe = &frames[frames.len() / 2];
    let (w, h, colors, mean) =
        verify::png_stats(&fs::read(probe)?).map_err(|e| format!("{}: {e}", probe.display()))?;
    if colors < 16 {
        return Err(format!(
            "frame {} is blank ({colors} colours): the render is empty",
            probe.display()
        )
        .into());
    }
    let first = frames[0]
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let pattern = first.rsplit_once('_').map_or(first.clone(), |(p, rest)| {
        format!("{p}_{}", rest.replace(|c: char| c.is_ascii_digit(), "#"))
    });
    println!(
        "rsepuh: {} frames {w}x{h} in assets/3d/{name} (mid frame: {colors} colours, luma {mean})",
        frames.len()
    );
    println!("rsepuh: use it in a scene as  \"overlay\": \"assets/3d/{name}/{pattern}\"");
    Ok(0)
}

fn gen(root: &Path, a: &mut Args) -> Result<u8> {
    let kind = a.first().ok_or(
        "usage: rsepuh gen image PROMPT [--out PATH] [--aspect 9:16] [--ref FILE] [--model M]",
    )?;
    if kind != "image" {
        return Err(if kind.contains("video") || kind.contains("veo") {
            "rsepuh never generates video (no Veo): motion is handcrafted in EffectCraft and Blender. Use `rsepuh gen image` for stills.".into()
        } else {
            format!("unknown generator `{kind}` (only `image`)").into()
        });
    }
    let model = gemini::normalise_model(
        &a.opt("--model")?
            .unwrap_or_else(|| gemini::DEFAULT_MODEL.into()),
    );
    gemini::guard_model(&model)?;
    let aspect = a.opt("--aspect")?;
    let env = a.opt("--env")?.map(PathBuf::from);
    let out = a.opt("--out")?;
    let refs = a.opts("--ref")?;
    let prompt = a.0.join(" ");
    if prompt.trim().is_empty() {
        return Err("give a prompt: rsepuh gen image \"a paper-cut mountain at dawn\" --out assets/gen/bg.png".into());
    }
    let key = gemini::find_key(env.as_deref()).ok_or("no Gemini key: set GEMINI_API_KEY (or GEMINI_KEY), or pass --env FILE. Everything else in rsepuh works without one.")?;
    let mut ref_data = Vec::new();
    for r in &refs {
        let p = Path::new(r);
        ref_data.push((
            gemini::mime_for(p)?.to_string(),
            fs::read(p).map_err(|e| format!("reference {r}: {e}"))?,
        ));
    }
    let body = gemini::request_body(&prompt, aspect.as_deref(), &ref_data);
    let img =
        gemini::generate(&key, &model, &body).map_err(|e| gemini::redact(&e.to_string(), &key))?;
    let mut path = match out {
        Some(o) => PathBuf::from(o),
        None => root.join("assets/gen").join(format!(
            "{}.png",
            spec::slug(&prompt.chars().take(32).collect::<String>())
        )),
    };
    let want = gemini::ext_for(&img.mime);
    if path.extension().is_none_or(|e| e != want) {
        path.set_extension(want);
    }
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    fs::write(&path, &img.bytes)?;
    let ledger = root.join("assets/gen/ledger.jsonl");
    fs::create_dir_all(root.join("assets/gen"))?;
    let line = json!({
        "file": path.strip_prefix(root).unwrap_or(&path).to_string_lossy(),
        "model": model, "prompt": prompt, "aspect": aspect, "refs": refs,
        "at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
    });
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger)?;
    use std::io::Write;
    writeln!(f, "{line}")?;
    println!(
        "rsepuh: {} ({}, {} KiB, {model})",
        path.display(),
        img.mime,
        img.bytes.len() / 1024
    );
    println!(
        "rsepuh: use it as  \"src\": \"{}\"",
        path.strip_prefix(root).unwrap_or(&path).display()
    );
    Ok(0)
}

// ------------------------------------------------------------------ verify, check

fn verify_cmd(a: &mut Args) -> Result<u8> {
    let p = PathBuf::from(a.first().ok_or("usage: rsepuh verify PATH")?);
    let findings = if p.is_dir() {
        verify::dir(&p)?
    } else {
        verify::file(&p).into_iter().collect()
    };
    if findings.is_empty() {
        return Err(format!(
            "nothing verifiable at {} (mp4, mov, png, json)",
            p.display()
        )
        .into());
    }
    let mut bad = 0;
    for f in &findings {
        println!(
            "[{}] {:<28} {}",
            if f.ok { " ok " } else { "FAIL" },
            f.file,
            f.detail
        );
        bad += usize::from(!f.ok);
    }
    println!("\n{} checked, {bad} failed", findings.len());
    Ok(u8::from(bad > 0))
}

fn check_cmd(a: &mut Args) -> Result<u8> {
    let which = a.opt("--platform")?.unwrap_or_else(|| "all".into());
    let p = PathBuf::from(
        a.first()
            .ok_or("usage: rsepuh check FILE.mp4 [--platform tiktok|reels|shorts|all]")?,
    );
    let m = verify::parse_mp4(&fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?)?;
    let targets: Vec<&verify::Platform> = if which == "all" {
        verify::PLATFORMS.iter().collect()
    } else {
        vec![verify::platform(&which).ok_or_else(|| {
            format!(
                "unknown platform `{which}` ({})",
                verify::PLATFORMS
                    .iter()
                    .map(|p| p.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?]
    };
    let mut any_bad = false;
    for pl in targets {
        println!("{}", pl.name);
        for (ok, msg) in verify::check_platform(&m, pl) {
            any_bad |= !ok;
            println!("  [{}] {msg}", if ok { " ok " } else { "FAIL" });
        }
    }
    println!("\nLimits checked 2026-11-06 against the platforms' API docs; confirm in the app before a launch.");
    Ok(u8::from(any_bad))
}

// ------------------------------------------------------------------ doctor, pv

fn have(cmd: &str, arg: &str) -> bool {
    Command::new(cmd)
        .arg(arg)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn doctor(remote: &Remote, with_blender: bool) -> Result<u8> {
    println!("local");
    for (n, ok, hint) in [
        ("rcargo", have("rcargo", "--version"), "curl -fsSL https://raw.githubusercontent.com/igun997/rcargo/main/install.sh | sh"),
        ("rblender", have("rblender", "--version"), "curl -fsSL https://raw.githubusercontent.com/igun997/rblender/main/install.sh | sh (only for 3D)"),
        ("curl", have("curl", "--version"), "needed for Gemini calls"),
        ("uv", have("uv", "--version"), "https://docs.astral.sh/uv/ (runs the free edge voice with a current edge-tts)"),
    ] {
        println!("  [{}] {n}{}", if ok { " ok " } else { "warn" }, if ok { String::new() } else { format!("  -> {hint}") });
    }
    let key = gemini::find_key(None).is_some();
    println!(
        "  [{}] Gemini key (optional: stills and the gemini voice){}",
        if key { " ok " } else { "none" },
        if key {
            ""
        } else {
            "  -> set GEMINI_API_KEY or put it in ./.env"
        }
    );
    if !have("rcargo", "--version") {
        return Ok(1);
    }
    println!("sandbox");
    let st = Command::new("rcargo")
        .args(["remote", "doctor"])
        .status()
        .map_err(Remote::spawn_err)?;
    if !st.success() && st.code() != Some(1) {
        return Ok(status(st));
    }
    remote.ensure()?;
    let st = remote.shell(
        "\"$HOME/effectcraft/bin/effectcraft-cli\" --empty info | head -1; echo \"cpus: $(nproc)\"; df -h . | tail -1 | awk '{print \"disk free: \" $4}'",
        true,
        true,
    )?;
    if with_blender {
        println!("blender");
        return Ok(status(
            remote
                .rblender(&["doctor"])
                .status()
                .map_err(blender_spawn_err)?,
        ));
    }
    Ok(status(st))
}

fn pv(root: &Path, remote: &Remote, a: &mut Args) -> Result<u8> {
    if !root.join("Cargo.toml").exists() {
        return Err("run `rsepuh init` in an empty folder first, then `rsepuh pv`".into());
    }
    let env = a.opt("--env")?;
    let key_present = gemini::find_key(env.as_deref().map(Path::new)).is_some();
    let engine = a
        .opt("--engine")?
        .unwrap_or_else(|| if key_present { "gemini" } else { "edge" }.into());
    let draft = a.flag("--draft");
    let mut v: Value = serde_json::from_str(assets::PV_SPEC)?;
    let key = gemini::find_key(env.as_deref().map(Path::new));

    // 1. the 3D seal (Blender through rblender); the PV still works without it
    let seal = root.join("assets/3d/seal/seal_0001.png");
    if !seal.is_file() {
        if have("rblender", "--version") {
            let mut ba = Args(vec![
                "medal".into(),
                "--out".into(),
                "seal".into(),
                "--size".into(),
                "560".into(),
                "--frames".into(),
                "60".into(),
                "--label".into(),
                "SEPUH".into(),
            ]);
            if blender(root, remote, &mut ba).is_err() || !seal.is_file() {
                eprintln!("rsepuh: 3D seal failed; continuing without it");
            }
        } else {
            eprintln!("rsepuh: rblender not installed; continuing without the 3D seal");
        }
    }
    // 2. a Gemini still for the background, when a key exists (Gemini may answer in JPEG)
    let find_bg = || {
        ["png", "jpg", "webp"]
            .iter()
            .map(|e| root.join(format!("assets/gen/pv-bg.{e}")))
            .find(|p| p.is_file())
    };
    if find_bg().is_none() {
        if key.is_some() {
            let mut ga = Args(vec![
                "image".into(),
                "Abstract warm paper-cut layers in deep violet and gold, soft light, empty centre for a title, no text, no people".into(),
                "--aspect".into(),
                "9:16".into(),
                "--out".into(),
                root.join("assets/gen/pv-bg.png").to_string_lossy().into_owned(),
            ]);
            if let Some(e) = &env {
                ga.0.push("--env".into());
                ga.0.push(e.clone());
            }
            if let Err(e) = gen(root, &mut ga) {
                eprintln!("rsepuh: Gemini still failed ({e}); continuing without it");
            }
        } else {
            eprintln!("rsepuh: no Gemini key; the PV uses a plain background (set GEMINI_API_KEY for a generated one)");
        }
    }
    let bg_rel = find_bg().map(|p| {
        p.strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .into_owned()
    });
    let scenes = v["scenes"].as_array_mut().ok_or("bad PV template")?;
    for sc in scenes.iter_mut() {
        if sc["type"] == "media" {
            match &bg_rel {
                Some(p) => sc["src"] = json!(p),
                None => {
                    sc.as_object_mut().map(|m| m.remove("src"));
                }
            }
            if !seal.is_file() {
                sc.as_object_mut().map(|m| m.remove("overlay"));
            }
        }
    }
    if engine == "edge" {
        // the template's Gemini delivery notes do not apply to the free voice
        v["voice"] = json!({ "engine": "edge", "lang": "en-US", "rate": "+8%" });
    }
    fs::create_dir_all(root.join("scenes"))?;
    let path = root.join("scenes/pv.json");
    fs::write(&path, serde_json::to_string_pretty(&v)? + "\n")?;
    // 3. the voice-over; without one the PV still renders silently
    let mut va = Args(vec!["pv".into()]);
    if let Some(e) = &env {
        va.0.push("--env".into());
        va.0.push(e.clone());
    }
    if let Err(e) = voice_cmd(root, &mut va) {
        eprintln!("rsepuh: voice failed ({e}); rendering the PV without sound");
        if let Some(sc) = v["scenes"].as_array_mut() {
            for s in sc.iter_mut() {
                s.as_object_mut().map(|m| m.remove("voice"));
            }
        }
        fs::write(&path, serde_json::to_string_pretty(&v)? + "\n")?;
    }
    // 4. the beat-grid music bed and sound effects, from the timeline the voice just fixed
    if v["music"]["gen"].is_string() {
        if let Err(e) = music_cmd(root, &mut Args(vec!["pv".into()])) {
            eprintln!("rsepuh: music failed ({e}); rendering without it");
            if let Some(m) = v.as_object_mut() {
                m.remove("music");
                m.remove("sfx");
            }
            fs::write(&path, serde_json::to_string_pretty(&v)? + "\n")?;
        }
    }
    let mut ra = Args(vec!["pv".into()]);
    if draft {
        ra.0.push("--draft".into());
    }
    render(root, remote, &mut ra)
}
