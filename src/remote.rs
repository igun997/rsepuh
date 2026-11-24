//! Remote execution through rcargo: instance selection, project sync and SSH are rcargo's job.
//! rsepuh adds "make sure EffectCraft is there", "build and render", "bring the results back".
//! Blender runs through rblender, which sits on the same rcargo instance.

use std::error::Error;
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use crate::assets;
use crate::util::{base64, base64_decode, gzip, sh_quote};
use crate::Result;

/// `bash -c` arguments above this travel badly over SSH and hit the kernel's per-argument cap.
pub const MAX_SCRIPT_BYTES: usize = 100_000;

pub struct Remote {
    pub instance: Option<String>,
    pub version: String,
}

pub struct RenderOpts<'a> {
    pub name: &'a str,
    /// Compositor script (spec and style injected).
    pub jsx: &'a str,
    /// Resolved spec and style for the motion writer.
    pub plan: &'a str,
    pub scenes: usize,
    pub frame_times: &'a [f64],
    pub draft: bool,
    pub bitrate_kbps: u32,
}

/// Names are interpolated inside double quotes, so only slug characters are allowed through.
fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Remote bash shared by render and frame: write the plan and compositor, run the Python motion
/// writer (one Lottie per scene), import those into EffectCraft, then build the composition.
fn build_steps(n: &str, jsx: &str, plan: &str, scenes: usize) -> String {
    let mut s = String::from("set -euo pipefail\n");
    s += "EC=\"$HOME/effectcraft/bin/effectcraft-cli\"\nD=\"$HOME/.rsepuh/build\"; mkdir -p \"$D\"\n";
    s += "command -v python3 >/dev/null || { echo 'rsepuh: python3 is required on the sandbox (the motion writer)' >&2; exit 1; }\n";
    for (ext, body) in [("jsx", jsx), ("plan.json", plan)] {
        s += &format!(
            "tmp=$(echo {} | base64 -d | gunzip)\nprintf '%s' \"${{tmp//__ROOT__/$PWD}}\" > \"$D/{n}.{ext}\"\n",
            base64(&gzip(body.as_bytes()))
        );
    }
    s += &format!(
        "OUT=\"renders/{n}\"; rm -rf \"$OUT\"; mkdir -p \"$OUT/frames\" \"$OUT/lottie\"\n"
    );
    s += &format!("python3 lottie/writer.py \"$D/{n}.plan.json\" \"$OUT/lottie\" >&2\n");
    let imports: String = (0..scenes)
        .map(|i| format!(" file.importLottie \"{{\\\"path\\\":\\\"$PWD/$OUT/lottie/scene-{i:02}.json\\\"}}\""))
        .collect();
    s += &format!(
        "\"$EC\" --empty --save-as \"$OUT/base.ecproj\" run{imports} > \"$OUT/import.log\"\n"
    );
    s += "awk '/\"warnings\": \\[\\]/{next} /\"warnings\": \\[/{f=1;next} f&&/^ *\\]/{f=0} f{print \"rsepuh: lottie import warning: \" $0}' \"$OUT/import.log\" >&2 || true\n";
    s += &format!("\"$EC\" --project \"$OUT/base.ecproj\" --save-as \"$OUT/{n}.ecproj\" script \"$D/{n}.jsx\" >&2\nrm -f \"$OUT/base.ecproj\"\n");
    s
}

/// Remote bash: build the project, render the MP4 and a few stills.
pub fn render_script(o: &RenderOpts) -> String {
    let n = safe_name(o.name);
    let mut s = build_steps(&n, o.jsx, o.plan, o.scenes);
    let res = if o.draft { " --resolution half" } else { "" };
    s += &format!(
        "\"$EC\" --project \"$OUT/{n}.ecproj\" render --comp Main --out \"$OUT/{n}.mp4\" --format h264 \
         --bitrate {}{res} --progress 2>&1 | awk '/^progress/ {{ if ($2 % 60 == 0) print \"  rendered \" $2 \"/\" $3 \" frames\"; next }} {{ print }}'\n",
        o.bitrate_kbps
    );
    for (i, t) in o.frame_times.iter().enumerate() {
        s += &format!(
            "\"$EC\" --project \"$OUT/{n}.ecproj\" render-frame --comp Main --time {t:.3} --scale 0.25 \
             --out \"$OUT/frames/f{i:02}.png\" >/dev/null\n"
        );
    }
    s
}

/// Remote bash for one preview frame.
pub fn frame_script(o: &RenderOpts, at: f64, scale: f64) -> String {
    let n = safe_name(o.name);
    let mut s = build_steps(&n, o.jsx, o.plan, o.scenes);
    s += &format!(
        "\"$EC\" --project \"$OUT/{n}.ecproj\" render-frame --comp Main --time {at:.3} --scale {scale} --out \"$OUT/frame.png\"\n"
    );
    s
}

impl Remote {
    fn rcargo(&self, no_sync: bool, quiet: bool) -> Command {
        let mut c = Command::new("rcargo");
        if let Some(i) = &self.instance {
            c.args(["-i", i]);
        }
        if quiet {
            c.arg("-q");
        }
        if no_sync {
            c.arg("--no-sync");
        }
        c.args(["remote", "shell", "--", "bash", "-c"]);
        c
    }

    pub fn spawn_err(e: std::io::Error) -> Box<dyn Error> {
        if e.kind() == std::io::ErrorKind::NotFound {
            "rcargo is not installed or not on PATH. Install: \
             curl -fsSL https://raw.githubusercontent.com/igun997/rcargo/main/install.sh | sh"
                .into()
        } else {
            e.into()
        }
    }

    /// Run a bash program on the remote workspace with inherited stdio.
    pub fn shell(&self, script: &str, no_sync: bool, quiet: bool) -> Result<ExitStatus> {
        if script.len() > MAX_SCRIPT_BYTES {
            return Err(format!(
                "generated remote script is {} KB (limit {} KB): shorten custom scene scripts or the spec",
                script.len() / 1000,
                MAX_SCRIPT_BYTES / 1000
            )
            .into());
        }
        self.rcargo(no_sync, quiet)
            .arg(script)
            .status()
            .map_err(Self::spawn_err)
    }

    fn probe(&self, script: &str) -> Result<bool> {
        Ok(self
            .rcargo(false, true)
            .arg(script)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(Self::spawn_err)?
            .success())
    }

    /// Install the EffectCraft CLI if the remote lacks it or has another version.
    pub fn ensure(&self) -> Result<()> {
        if self.probe(&assets::check_script(&self.version))? {
            return Ok(());
        }
        eprintln!(
            "rsepuh: installing EffectCraft {} on the remote machine (first use, under a minute)",
            self.version
        );
        let st = self.shell(&assets::deploy_script(&self.version), false, false)?;
        if !st.success() {
            return Err(format!("remote provisioning failed ({st})").into());
        }
        Ok(())
    }

    /// Copy a remote directory into `dest`. The archive travels as gzip + base64 because
    /// rcargo's shell stream is text-oriented.
    pub fn pull(&self, remote_dir: &str, dest: &Path) -> Result<u64> {
        std::fs::create_dir_all(dest)?;
        let script = format!(
            "cd {} 2>/dev/null && tar -czf - . | base64 -w0",
            sh_quote(remote_dir)
        );
        let res = self
            .rcargo(true, true)
            .arg(script)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .output()
            .map_err(Self::spawn_err)?;
        if !res.status.success() {
            return Err(format!("remote has no `{remote_dir}` (nothing was produced)").into());
        }
        let tarball =
            base64_decode(&res.stdout).ok_or("corrupt transfer from remote (bad base64)")?;
        let mut sink = Command::new("tar")
            .args(["-xzf", "-", "-C"])
            .arg(dest)
            .stdin(Stdio::piped())
            .spawn()?;
        sink.stdin.take().ok_or("no stdin")?.write_all(&tarball)?;
        if !sink.wait()?.success() {
            return Err("failed to unpack results".into());
        }
        Ok(dir_size(dest))
    }

    /// `rblender` against the same instance. Output is captured so rsepuh controls what is shown.
    pub fn rblender(&self, args: &[&str]) -> Command {
        let mut c = Command::new("rblender");
        if let Some(i) = &self.instance {
            c.args(["-i", i]);
        }
        c.args(args);
        c
    }
}

pub fn dir_size(p: &Path) -> u64 {
    std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts<'a>(
        name: &'a str,
        jsx: &'a str,
        frames: &'a [f64],
        draft: bool,
        kbps: u32,
    ) -> RenderOpts<'a> {
        RenderOpts {
            name,
            jsx,
            plan: "{\"spec\":{}}",
            scenes: 3,
            frame_times: frames,
            draft,
            bitrate_kbps: kbps,
        }
    }

    #[test]
    fn render_script_writes_lottie_then_imports_then_builds_then_renders() {
        let s = render_script(&opts(
            "my video",
            "writeLn('x\"; rm -rf /')",
            &[0.5, 2.0],
            true,
            8000,
        ));
        assert!(!s.contains("rm -rf /"), "script text must travel as base64");
        let order = [
            "python3 lottie/writer.py",
            "file.importLottie",
            "--save-as \"$OUT/my-video.ecproj\" script",
            " render --comp Main",
            "render-frame",
        ];
        let mut last = 0;
        for needle in order {
            let at = s.find(needle).unwrap_or_else(|| panic!("missing {needle}"));
            assert!(at >= last, "{needle} out of order");
            last = at;
        }
        for i in 0..3 {
            assert!(
                s.contains(&format!("scene-{i:02}.json")),
                "scene {i} is imported"
            );
        }
        assert!(s.contains("--bitrate 8000 --resolution half"));
        assert!(s.contains("f00.png") && s.contains("f01.png"));
        assert!(s.contains("${tmp//__ROOT__/$PWD}") && s.contains("base64 -d | gunzip"));
        assert!(s.contains("python3 is required"));
    }

    #[test]
    fn full_quality_render_has_no_resolution_flag_and_frame_script_renders_one_frame() {
        assert!(!render_script(&opts("a", "", &[], false, 12000)).contains("--resolution"));
        let f = frame_script(&opts("a", "", &[], false, 1), 1.5, 0.5);
        assert!(f.contains("--time 1.500 --scale 0.5") && !f.contains(" render --comp Main"));
    }

    #[test]
    fn oversized_scripts_are_refused_before_ssh() {
        let r = Remote {
            instance: None,
            version: "x".into(),
        };
        let big = "a".repeat(MAX_SCRIPT_BYTES + 1);
        assert!(r
            .shell(&big, true, true)
            .unwrap_err()
            .to_string()
            .contains("shorten"));
    }
}
