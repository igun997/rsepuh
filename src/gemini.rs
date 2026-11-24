//! Optional Gemini image generation for assets (backgrounds, textures, stickers, reference art).
//! Video generation is refused on purpose: rsepuh is for handcrafted motion, so Veo and any
//! other video model never go through here.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use crate::util::{base64, base64_decode};
use crate::Result;

pub const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
/// Verified for `generateContent` image output by rveditor on 2026-11-19.
pub const DEFAULT_MODEL: &str = "gemini-3.1-flash-image";
pub const KEY_VARS: [&str; 2] = ["GEMINI_API_KEY", "GEMINI_KEY"];

pub fn normalise_model(m: &str) -> String {
    m.trim().trim_start_matches("models/").to_string()
}

/// Refuse video models. Anything with `veo` or `video` in its name is rejected.
pub fn guard_model(m: &str) -> Result<()> {
    let l = m.to_ascii_lowercase();
    if l.contains("veo") || l.contains("video") {
        return Err(format!(
            "`{m}` is a video model. rsepuh never generates video (no Veo): motion is handcrafted \
             in EffectCraft and Blender. Gemini is only for still assets."
        )
        .into());
    }
    Ok(())
}

/// Minimal `.env` reader: `KEY=value`, optional `export`, optional quotes, `#` comments.
pub fn parse_env(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            let l = l.strip_prefix("export ").unwrap_or(l);
            let (k, v) = l.split_once('=')?;
            let v = v.trim();
            let v = v
                .strip_prefix('"')
                .and_then(|x| x.strip_suffix('"'))
                .or_else(|| v.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')))
                .unwrap_or(v);
            Some((k.trim().to_string(), v.to_string()))
        })
        .collect()
}

/// Find the key: process environment first, then `--env FILE`, then `./.env`.
pub fn find_key(env_file: Option<&Path>) -> Option<String> {
    for k in KEY_VARS {
        if let Ok(v) = std::env::var(k) {
            if !v.trim().is_empty() {
                return Some(v.trim().to_string());
            }
        }
    }
    let mut files: Vec<PathBuf> = Vec::new();
    if let Some(f) = env_file {
        files.push(f.to_path_buf());
    }
    files.push(PathBuf::from(".env"));
    for f in files {
        if let Ok(text) = fs::read_to_string(&f) {
            let vars = parse_env(&text);
            for k in KEY_VARS {
                if let Some((_, v)) = vars.iter().find(|(n, _)| n == k) {
                    if !v.is_empty() {
                        return Some(v.clone());
                    }
                }
            }
        }
    }
    None
}

pub fn request_body(prompt: &str, aspect: Option<&str>, refs: &[(String, Vec<u8>)]) -> Value {
    let mut parts: Vec<Value> = refs
        .iter()
        .map(|(mime, bytes)| json!({"inlineData": {"mimeType": mime, "data": base64(bytes)}}))
        .collect();
    parts.push(json!({"text": prompt}));
    let mut gen = json!({"responseModalities": ["IMAGE"]});
    if let Some(a) = aspect {
        gen["imageConfig"] = json!({"aspectRatio": a});
    }
    json!({"contents": [{"role": "user", "parts": parts}], "generationConfig": gen})
}

#[derive(Debug)]
pub struct Image {
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// Pull the first image out of a `generateContent` reply, or explain why there is none.
pub fn extract_image(resp: &Value) -> Result<Image> {
    if let Some(b) = resp["promptFeedback"]["blockReason"].as_str() {
        return Err(format!("Gemini blocked the prompt ({b}). Rephrase it.").into());
    }
    let cand = resp["candidates"]
        .get(0)
        .ok_or("Gemini returned no candidates")?;
    let mut text = String::new();
    for p in cand["content"]["parts"].as_array().into_iter().flatten() {
        let inline = p.get("inlineData").or_else(|| p.get("inline_data"));
        if let Some(d) = inline {
            let mime = d["mimeType"]
                .as_str()
                .or_else(|| d["mime_type"].as_str())
                .unwrap_or("image/png");
            let data = d["data"].as_str().ok_or("image part without data")?;
            let bytes = base64_decode(data.as_bytes()).ok_or("image data is not valid base64")?;
            return Ok(Image {
                mime: mime.to_string(),
                bytes,
            });
        }
        if let Some(t) = p["text"].as_str() {
            text += t;
        }
    }
    let reason = cand["finishReason"].as_str().unwrap_or("none");
    Err(format!(
        "Gemini returned no image (finishReason {reason}){}",
        if text.is_empty() {
            String::new()
        } else {
            format!(": {}", text.chars().take(240).collect::<String>())
        }
    )
    .into())
}

pub fn ext_for(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => "png",
    }
}

pub fn mime_for(path: &Path) -> Result<&'static str> {
    match path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Ok("image/png"),
        Some("jpg" | "jpeg") => Ok("image/jpeg"),
        Some("webp") => Ok("image/webp"),
        _ => Err(format!("reference {} must be png, jpg or webp", path.display()).into()),
    }
}

/// One `generateContent` call through curl, returning the parsed reply. The key goes to curl on
/// stdin so it never shows in `ps` or in any error text.
pub fn post_json(key: &str, model: &str, body: &Value) -> Result<Value> {
    guard_model(model)?;
    let dir = std::env::temp_dir().join(format!(
        "rsepuh-gen-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    fs::create_dir_all(&dir)?;
    let (req, resp) = (dir.join("req.json"), dir.join("resp.json"));
    fs::write(&req, serde_json::to_vec(body)?)?;
    let result = (|| -> Result<Value> {
        let mut child = Command::new("curl")
            .args([
                "-sS",
                "--max-time",
                "240",
                "-X",
                "POST",
                "-H",
                "Content-Type: application/json",
            ])
            .args(["--config", "-", "--data-binary"])
            .arg(format!("@{}", req.display()))
            .args(["-o"])
            .arg(&resp)
            .args(["-w", "%{http_code}"])
            .arg(format!("{BASE_URL}/models/{model}:generateContent"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "curl is required for Gemini calls".to_string()
                } else {
                    e.to_string()
                }
            })?;
        child
            .stdin
            .take()
            .ok_or("no stdin")?
            .write_all(format!("header = \"x-goog-api-key: {key}\"\n").as_bytes())?;
        let out = child.wait_with_output()?;
        let code = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !out.status.success() {
            return Err(redact(
                &format!(
                    "curl failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
                key,
            )
            .into());
        }
        let text = fs::read_to_string(&resp).unwrap_or_default();
        let json: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if code != "200" {
            let msg = json["error"]["message"].as_str().unwrap_or(&text);
            return Err(redact(
                &format!(
                    "Gemini HTTP {code}: {}{}",
                    msg.chars().take(300).collect::<String>(),
                    if code == "400" || code == "403" {
                        " (check GEMINI_API_KEY / GEMINI_KEY and the model or voice name)"
                    } else {
                        ""
                    }
                ),
                key,
            )
            .into());
        }
        Ok(json)
    })();
    let _ = fs::remove_dir_all(&dir);
    result
}

pub fn generate(key: &str, model: &str, body: &Value) -> Result<Image> {
    extract_image(&post_json(key, model, body)?)
}

/// First inline audio part of a reply: (mime type, bytes).
pub fn extract_audio(resp: &Value) -> Result<(String, Vec<u8>)> {
    if let Some(b) = resp["promptFeedback"]["blockReason"].as_str() {
        return Err(format!("Gemini blocked the text ({b}). Rephrase it.").into());
    }
    let cand = resp["candidates"]
        .get(0)
        .ok_or("Gemini returned no candidates")?;
    for p in cand["content"]["parts"].as_array().into_iter().flatten() {
        if let Some(d) = p.get("inlineData").or_else(|| p.get("inline_data")) {
            let mime = d["mimeType"]
                .as_str()
                .or_else(|| d["mime_type"].as_str())
                .unwrap_or("audio/L16;rate=24000");
            let data = d["data"].as_str().ok_or("audio part without data")?;
            let bytes = base64_decode(data.as_bytes()).ok_or("audio data is not valid base64")?;
            return Ok((mime.to_string(), bytes));
        }
    }
    Err(format!(
        "Gemini returned no audio (finishReason {})",
        cand["finishReason"].as_str().unwrap_or("none")
    )
    .into())
}

pub fn redact(s: &str, key: &str) -> String {
    if key.is_empty() {
        s.to_string()
    } else {
        s.replace(key, "<redacted>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_models_are_refused_everything_else_passes() {
        for m in [
            "veo-3.1",
            "models/VEO-2",
            "gemini-veo-fast",
            "some-video-model",
        ] {
            assert!(
                guard_model(m).unwrap_err().to_string().contains("no Veo"),
                "{m}"
            );
        }
        for m in [
            DEFAULT_MODEL,
            "gemini-2.5-flash-image",
            "imagen-4.0-generate",
        ] {
            assert!(guard_model(m).is_ok(), "{m}");
        }
        assert_eq!(normalise_model(" models/x "), "x");
    }

    #[test]
    fn env_files_parse_quotes_comments_and_export() {
        let v = parse_env(
            "# c\nURL=http://x\nexport GEMINI_KEY=\"abc\"\nA='b'\n\nBAD LINE\nTAVILY=t=u",
        );
        assert_eq!(v[1], ("GEMINI_KEY".into(), "abc".into()));
        assert_eq!(v[2], ("A".into(), "b".into()));
        assert_eq!(v[3], ("TAVILY".into(), "t=u".into()));
        assert_eq!(v.len(), 4);
    }

    #[test]
    fn request_has_refs_first_then_prompt_and_optional_aspect() {
        let b = request_body(
            "a cat",
            Some("9:16"),
            &[("image/png".into(), vec![1, 2, 3])],
        );
        let parts = b["contents"][0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["inlineData"]["data"], "AQID");
        assert_eq!(parts[1]["text"], "a cat");
        assert_eq!(b["generationConfig"]["imageConfig"]["aspectRatio"], "9:16");
        assert!(request_body("x", None, &[])["generationConfig"]["imageConfig"].is_null());
    }

    #[test]
    fn extracts_images_and_explains_refusals() {
        let ok = json!({"candidates":[{"content":{"parts":[{"text":"hi"},{"inlineData":{"mimeType":"image/jpeg","data":"AQID"}}]}}]});
        let img = extract_image(&ok).unwrap();
        assert_eq!(
            (img.mime.as_str(), img.bytes.as_slice()),
            ("image/jpeg", &[1u8, 2, 3][..])
        );
        let refused = json!({"candidates":[{"finishReason":"IMAGE_SAFETY"}]});
        assert!(extract_image(&refused)
            .unwrap_err()
            .to_string()
            .contains("IMAGE_SAFETY"));
        let blocked = json!({"promptFeedback":{"blockReason":"SAFETY"}});
        assert!(extract_image(&blocked)
            .unwrap_err()
            .to_string()
            .contains("blocked"));
        assert!(extract_image(&json!({})).is_err());
    }

    #[test]
    fn redaction_removes_the_key() {
        assert_eq!(
            redact("bad key SECRET here", "SECRET"),
            "bad key <redacted> here"
        );
    }
}
