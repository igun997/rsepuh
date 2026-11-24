//! Everything rsepuh ships inside the binary: the motion engine, themes, Blender scripts, the
//! EffectCraft installer and the agent skills.

use crate::util::{base64, fnv1a, sh_quote};

pub const EFFECTCRAFT_VERSION: &str = "0.25.1";

/// The compositor: backgrounds, footage, the Lottie scenes, camera, subtitles and audio.
pub const COMPOSITOR_JSX: &str = include_str!("../styles/compositor.jsx");

/// The motion writer (Python, standard library only) and its fonts. `rsepuh init` and every render
/// copy any that are missing into the project's `lottie/` folder, where you can edit them.
pub const LOTTIE_FILES: &[(&str, &str)] = &[
    ("lottie/kit.py", include_str!("../lottie/kit.py")),
    ("lottie/rs.py", include_str!("../lottie/rs.py")),
    ("lottie/writer.py", include_str!("../lottie/writer.py")),
    (
        "lottie/scenes/hook.py",
        include_str!("../lottie/scenes/hook.py"),
    ),
    (
        "lottie/scenes/title.py",
        include_str!("../lottie/scenes/title.py"),
    ),
    (
        "lottie/scenes/chips.py",
        include_str!("../lottie/scenes/chips.py"),
    ),
    (
        "lottie/scenes/stat.py",
        include_str!("../lottie/scenes/stat.py"),
    ),
    (
        "lottie/scenes/cta.py",
        include_str!("../lottie/scenes/cta.py"),
    ),
    (
        "lottie/scenes/caption.py",
        include_str!("../lottie/scenes/caption.py"),
    ),
    (
        "lottie/scenes/terminal.py",
        include_str!("../lottie/scenes/terminal.py"),
    ),
    (
        "lottie/scenes/code.py",
        include_str!("../lottie/scenes/code.py"),
    ),
    (
        "lottie/scenes/media.py",
        include_str!("../lottie/scenes/media.py"),
    ),
    (
        "lottie/fonts/inter-bold.json",
        include_str!("../lottie/fonts/inter-bold.json"),
    ),
    (
        "lottie/fonts/jetbrains-mono.json",
        include_str!("../lottie/fonts/jetbrains-mono.json"),
    ),
    (
        "lottie/fonts/Inter-LICENSE.txt",
        include_str!("../lottie/fonts/Inter-LICENSE.txt"),
    ),
    (
        "lottie/fonts/JetBrainsMono-OFL.txt",
        include_str!("../lottie/fonts/JetBrainsMono-OFL.txt"),
    ),
];
pub const INSTALL_EFFECTCRAFT: &str = include_str!("../scripts/install-effectcraft.sh");

pub const THEMES: &[(&str, &str)] = &[
    ("punch", include_str!("../styles/punch.json")),
    ("smooth", include_str!("../styles/smooth.json")),
    ("neon", include_str!("../styles/neon.json")),
    ("clean", include_str!("../styles/clean.json")),
];

/// Blender scripts users can read and edit; `rsepuh blender NAME` copies one into `blender/`.
pub const BLENDER_SCRIPTS: &[(&str, &str)] = &[
    ("stage", include_str!("../blender/stage.py")),
    ("text3d", include_str!("../blender/text3d.py")),
    ("medal", include_str!("../blender/medal.py")),
];

pub const SKILLS: &[(&str, &str)] = &[
    ("rsepuh", include_str!("../skills/rsepuh/SKILL.md")),
    (
        "rsepuh-lottie",
        include_str!("../skills/rsepuh-lottie/SKILL.md"),
    ),
    (
        "rsepuh-video-editing",
        include_str!("../skills/rsepuh-video-editing/SKILL.md"),
    ),
    (
        "rsepuh-subtitles",
        include_str!("../skills/rsepuh-subtitles/SKILL.md"),
    ),
    (
        "rsepuh-voiceover",
        include_str!("../skills/rsepuh-voiceover/SKILL.md"),
    ),
    (
        "rsepuh-publishing",
        include_str!("../skills/rsepuh-publishing/SKILL.md"),
    ),
];

pub const PV_SPEC: &str = include_str!("../examples/pv/pv.json");

/// Changes with the EffectCraft version or the installer, so a new rsepuh re-provisions itself.
pub fn stamp(version: &str) -> String {
    format!(
        "{version}-{:016x}",
        fnv1a(&[version.as_bytes(), INSTALL_EFFECTCRAFT.as_bytes()])
    )
}

/// Remote bash: exit 0 iff the EffectCraft CLI is installed and current.
pub fn check_script(version: &str) -> String {
    format!(
        "[ -x \"$HOME/effectcraft/bin/effectcraft-cli\" ] && [ \"$(cat \"$HOME/.rsepuh/stamp\" 2>/dev/null)\" = {} ]",
        sh_quote(&stamp(version))
    )
}

/// Remote bash: install EffectCraft (idempotent) and record the stamp.
pub fn deploy_script(version: &str) -> String {
    format!(
        "set -euo pipefail\nmkdir -p \"$HOME/.rsepuh\"\necho {} | base64 -d > \"$HOME/.rsepuh/install-effectcraft.sh\"\n\
         EFFECTCRAFT_VERSION={} bash \"$HOME/.rsepuh/install-effectcraft.sh\" >&2\necho {} > \"$HOME/.rsepuh/stamp\"\n",
        base64(INSTALL_EFFECTCRAFT.as_bytes()),
        sh_quote(version),
        sh_quote(&stamp(version))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_depends_on_version() {
        assert_ne!(stamp("0.25.1"), stamp("0.25.2"));
        assert!(stamp("0.25.1").starts_with("0.25.1-"));
    }

    #[test]
    fn every_theme_is_valid_and_complete() {
        let need = [
            "bg",
            "bg2",
            "fg",
            "muted",
            "accent",
            "accent2",
            "onAccent",
            "panel",
            "font",
            "mono",
            "caps",
            "motion",
            "pop",
            "popDur",
            "stagger",
            "shake",
            "transition",
        ];
        for (name, body) in THEMES {
            let v: serde_json::Value = serde_json::from_str(body).expect(name);
            assert_eq!(v["name"], *name);
            for k in need {
                assert!(!v[k].is_null(), "theme {name} lacks {k}");
            }
        }
    }

    #[test]
    fn skills_have_front_matter_naming_themselves() {
        for (name, body) in SKILLS {
            assert!(body.starts_with("---\nname: "), "{name}");
            assert!(body.contains(&format!("name: {name}\n")), "{name}");
            assert!(body.contains("description: "), "{name}");
        }
    }

    #[test]
    fn deploy_script_embeds_the_installer() {
        let s = deploy_script(EFFECTCRAFT_VERSION);
        assert!(s.contains("install-effectcraft.sh") && s.contains("EFFECTCRAFT_VERSION=0.25.1"));
    }
}
