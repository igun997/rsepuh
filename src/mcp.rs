//! MCP server over stdio (newline-delimited JSON-RPC 2.0). Each tool runs this same binary and
//! returns what the command printed, so an agent gets exactly what a human gets.

use std::io::{BufRead, Write};
use std::process::Command;

use serde_json::{json, Value};

use crate::Result;

const PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "rsepuh makes UGC-style motion videos (hooks, kinetic captions, stickers, \
terminal and stat scenes, call to action) with EffectCraft, optional Blender 3D and optional Gemini stills, \
rendered on a remote sandbox. Motion is handcrafted: there is no video generation and never will be (no Veo). \
Workflow: rsepuh_styles to see what exists, rsepuh_new for a starter spec, edit scenes/NAME.json, \
rsepuh_voice if scenes have `voice` lines, rsepuh_render with draft=true to iterate, then without draft for the \
final. Look at the contact sheet PNG after each render. Never pass or print API keys.";

fn tool(name: &str, desc: &str, props: Value, required: &[&str]) -> Value {
    json!({
        "name": name, "description": desc,
        "inputSchema": {"type": "object", "properties": props, "required": required}
    })
}

pub fn tools() -> Vec<Value> {
    let spec = json!({"type": "string", "description": "scenes/NAME.json or NAME"});
    vec![
        tool("rsepuh_styles", "List motion styles, scene types, subtitle styles and voice engines.", json!({}), &[]),
        tool("rsepuh_doctor", "Check rcargo, the sandbox, EffectCraft, voice engines and keys.", json!({"blender": {"type": "boolean"}}), &[]),
        tool("rsepuh_new", "Write a starter spec scenes/NAME.json.", json!({
            "name": {"type": "string"}, "style": {"type": "string"}, "aspect": {"type": "string", "description": "9:16, 16:9, 1:1, 4:5, 3:4"},
            "text": {"type": "string", "description": "hook words"}, "force": {"type": "boolean"}}), &["name"]),
        tool("rsepuh_voice", "Speak every scene's `voice` line (engine edge is free, gemini needs a key) and time the words. Re-run after changing a voice line.", json!({"spec": spec, "force": {"type": "boolean"}}), &["spec"]),
        tool("rsepuh_music", "Write a handcrafted music bed and sound effects on the spec's beat grid (needs bpm in the spec).", json!({"spec": spec, "style": {"type": "string", "enum": ["house", "lofi"]}}), &["spec"]),
        tool("rsepuh_render", "Build and render a spec on the sandbox; returns the MP4 path, contact sheet and platform fit. Use draft=true while iterating.", json!({
            "spec": spec, "draft": {"type": "boolean"}, "bitrate": {"type": "integer", "description": "kbps"}}), &["spec"]),
        tool("rsepuh_frame", "Render one preview frame (PNG) at a time in seconds.", json!({"spec": spec, "at": {"type": "number"}, "scale": {"type": "number"}}), &["spec", "at"]),
        tool("rsepuh_subs", "Write SRT/VTT sidecar subtitle files for a spec.", json!({"spec": spec, "format": {"type": "string", "enum": ["srt", "vtt", "both"]}}), &["spec"]),
        tool("rsepuh_blender", "Run a Blender script (text3d, medal or blender/NAME.py) on the sandbox; the PNG sequence lands in assets/3d/OUT.", json!({
            "script": {"type": "string"}, "out": {"type": "string"}, "args": {"type": "array", "items": {"type": "string"}}}), &["script"]),
        tool("rsepuh_gen_image", "Generate a still image with Gemini (needs a key). Never video.", json!({
            "prompt": {"type": "string"}, "out": {"type": "string"}, "aspect": {"type": "string"},
            "refs": {"type": "array", "items": {"type": "string"}}, "model": {"type": "string"}}), &["prompt"]),
        tool("rsepuh_verify", "Prove a render is real: parse the MP4, decode PNGs.", json!({"path": {"type": "string"}}), &["path"]),
        tool("rsepuh_check", "Test an MP4 against TikTok, Reels and Shorts upload limits.", json!({"file": {"type": "string"}, "platform": {"type": "string"}}), &["file"]),
    ]
}

fn s(args: &Value, k: &str) -> Option<String> {
    args[k].as_str().map(str::to_string)
}

fn req(args: &Value, k: &str) -> Result<String> {
    s(args, k).ok_or_else(|| format!("missing required argument `{k}`").into())
}

/// The command line a tool call stands for.
pub fn argv_for(name: &str, a: &Value) -> Result<Vec<String>> {
    let mut v: Vec<String> = Vec::new();
    let mut push = |x: &str| v.push(x.to_string());
    match name {
        "rsepuh_styles" => push("styles"),
        "rsepuh_doctor" => {
            push("doctor");
            if a["blender"].as_bool() == Some(true) {
                push("--blender");
            }
        }
        "rsepuh_new" => {
            push("new");
            push(&req(a, "name")?);
            for (k, f) in [
                ("style", "--style"),
                ("aspect", "--aspect"),
                ("text", "--text"),
            ] {
                if let Some(x) = s(a, k) {
                    push(f);
                    push(&x);
                }
            }
            if a["force"].as_bool() == Some(true) {
                push("--force");
            }
        }
        "rsepuh_voice" => {
            push("voice");
            push(&req(a, "spec")?);
            if a["force"].as_bool() == Some(true) {
                push("--force");
            }
        }
        "rsepuh_music" => {
            push("music");
            push(&req(a, "spec")?);
            if let Some(st) = s(a, "style") {
                push("--style");
                push(&st);
            }
        }
        "rsepuh_render" => {
            push("render");
            push(&req(a, "spec")?);
            if a["draft"].as_bool() == Some(true) {
                push("--draft");
            }
            if let Some(b) = a["bitrate"].as_u64() {
                push("--bitrate");
                push(&b.to_string());
            }
        }
        "rsepuh_frame" => {
            push("frame");
            push(&req(a, "spec")?);
            push("--at");
            push(
                &a["at"]
                    .as_f64()
                    .ok_or("missing required number `at`")?
                    .to_string(),
            );
            if let Some(sc) = a["scale"].as_f64() {
                push("--scale");
                push(&sc.to_string());
            }
        }
        "rsepuh_subs" => {
            push("subs");
            push(&req(a, "spec")?);
            if let Some(f) = s(a, "format") {
                push("--format");
                push(&f);
            }
        }
        "rsepuh_blender" => {
            push("blender");
            if let Some(o) = s(a, "out") {
                push("--out");
                push(&o);
            }
            push(&req(a, "script")?);
            for x in a["args"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                push(x);
            }
        }
        "rsepuh_gen_image" => {
            push("gen");
            push("image");
            for (k, f) in [
                ("out", "--out"),
                ("aspect", "--aspect"),
                ("model", "--model"),
            ] {
                if let Some(x) = s(a, k) {
                    push(f);
                    push(&x);
                }
            }
            for r in a["refs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                push("--ref");
                push(r);
            }
            push(&req(a, "prompt")?);
        }
        "rsepuh_verify" => {
            push("verify");
            push(&req(a, "path")?);
        }
        "rsepuh_check" => {
            push("check");
            if let Some(p) = s(a, "platform") {
                push("--platform");
                push(&p);
            }
            push(&req(a, "file")?);
        }
        other => return Err(format!("unknown tool `{other}`").into()),
    }
    Ok(v)
}

fn call(name: &str, args: &Value, instance: &Option<String>) -> Value {
    let argv = match argv_for(name, args) {
        Ok(a) => a,
        Err(e) => {
            return json!({"content": [{"type": "text", "text": e.to_string()}], "isError": true})
        }
    };
    let exe = std::env::current_exe().unwrap_or_else(|_| "rsepuh".into());
    let mut c = Command::new(exe);
    if let Some(i) = instance {
        c.args(["-i", i]);
    }
    match c.args(&argv).stdin(std::process::Stdio::null()).output() {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.trim().is_empty() {
                text += &format!("\n{err}");
            }
            json!({"content": [{"type": "text", "text": text.trim()}], "isError": !o.status.success()})
        }
        Err(e) => {
            json!({"content": [{"type": "text", "text": format!("could not run rsepuh: {e}")}], "isError": true})
        }
    }
}

pub fn handle(msg: &Value, instance: &Option<String>) -> Option<Value> {
    let method = msg["method"].as_str()?;
    let id = msg.get("id").cloned();
    let params = &msg["params"];
    let result: std::result::Result<Value, (i64, String)> = match method {
        "initialize" => Ok(json!({
            "protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL),
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": "rsepuh", "version": env!("CARGO_PKG_VERSION")},
            "instructions": INSTRUCTIONS,
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => Ok(call(
            params["name"].as_str().unwrap_or(""),
            &params["arguments"],
            instance,
        )),
        m if m.starts_with("notifications/") => return None,
        other => Err((-32601, format!("method not found: {other}"))),
    };
    let id = id?;
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err((code, message)) => {
            json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
        }
    })
}

pub fn serve(instance: Option<String>) -> Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let resp = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&msg, &instance),
            Err(e) => Some(
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}}),
            ),
        };
        if let Some(r) = resp {
            writeln!(out, "{r}")?;
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_tool_maps_to_a_command_with_its_required_arguments() {
        let sample = json!({
            "name": "x", "spec": "pv", "at": 1.5, "script": "medal", "prompt": "p", "path": "a", "file": "b.mp4"
        });
        for t in tools() {
            let name = t["name"].as_str().unwrap();
            let argv = argv_for(name, &sample).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!argv.is_empty(), "{name}");
            for r in t["inputSchema"]["required"].as_array().unwrap() {
                let k = r.as_str().unwrap();
                assert!(
                    argv_for(name, &json!({})).is_err() || k.is_empty(),
                    "{name} must reject missing {k}"
                );
            }
        }
    }

    #[test]
    fn arguments_become_flags_in_the_right_order() {
        let argv = argv_for(
            "rsepuh_render",
            &json!({"spec": "pv", "draft": true, "bitrate": 4000}),
        )
        .unwrap();
        assert_eq!(argv, ["render", "pv", "--draft", "--bitrate", "4000"]);
        let argv = argv_for(
            "rsepuh_gen_image",
            &json!({"prompt": "a b", "aspect": "9:16", "refs": ["r.png"]}),
        )
        .unwrap();
        assert_eq!(
            argv,
            ["gen", "image", "--aspect", "9:16", "--ref", "r.png", "a b"]
        );
        let argv = argv_for(
            "rsepuh_blender",
            &json!({"script": "medal", "out": "seal", "args": ["--color", "silver"]}),
        )
        .unwrap();
        assert_eq!(
            argv,
            ["blender", "--out", "seal", "medal", "--color", "silver"]
        );
        assert!(argv_for("rsepuh_nope", &json!({})).is_err());
    }

    #[test]
    fn protocol_handshake_lists_tools_and_reports_unknown_methods() {
        let init = handle(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
            &None,
        )
        .unwrap();
        assert_eq!(init["result"]["serverInfo"]["name"], "rsepuh");
        assert!(init["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("no Veo"));
        let list = handle(
            &json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            &None,
        )
        .unwrap();
        assert_eq!(
            list["result"]["tools"].as_array().unwrap().len(),
            tools().len()
        );
        assert!(handle(
            &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            &None
        )
        .is_none());
        let bad = handle(&json!({"jsonrpc": "2.0", "id": 3, "method": "nope"}), &None).unwrap();
        assert_eq!(bad["error"]["code"], -32601);
        let miss = handle(&json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "rsepuh_render", "arguments": {}}}), &None).unwrap();
        assert_eq!(miss["result"]["isError"], true);
    }
}
