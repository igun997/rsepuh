//! The motion writer is Python that runs on the sandbox. This runs it on whatever machine runs the
//! tests (CI, or a sandbox through rcargo) and checks the Lottie it writes for every scene type.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use rsepuh::assets::LOTTIE_FILES;
use serde_json::{json, Value};

fn have_python() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn project(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rsepuh-writer-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    for (rel, body) in LOTTIE_FILES {
        let p = d.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }
    d
}

fn plan(scenes: Value) -> Value {
    let theme: Value = serde_json::from_str(
        rsepuh::assets::THEMES
            .iter()
            .find(|(n, _)| *n == "punch")
            .unwrap()
            .1,
    )
    .unwrap();
    json!({"spec": {"name": "t", "size": [1080, 1920], "fps": 30, "beat": 0.5, "scenes": scenes}, "theme": theme})
}

fn run(d: &PathBuf, plan: &Value) -> (bool, String, PathBuf) {
    let pf = d.join("plan.json");
    fs::write(&pf, plan.to_string()).unwrap();
    let out = d.join("out");
    let o = Command::new("python3")
        .current_dir(d)
        .args([
            "lottie/writer.py",
            pf.to_str().unwrap(),
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    (
        o.status.success(),
        String::from_utf8_lossy(&o.stderr).into_owned() + &String::from_utf8_lossy(&o.stdout),
        out,
    )
}

#[test]
fn every_builtin_scene_type_writes_valid_lottie_of_the_right_size_and_length() {
    if !have_python() {
        eprintln!("python3 not found: skipping");
        return;
    }
    let scenes = json!([
        {"type": "hook", "dur": 3.0, "words": ["EDIT", "LIKE", "A VETERAN"], "wordT": [0.0, 0.8, 1.5], "sub": "Edit layaknya sepuh"},
        {"type": "title", "dur": 4.0, "lines": ["It synchronizes", "React with", "the outside world"], "sub": "really"},
        {"type": "chips", "dur": 6.0, "title": "Three", "items": ["One", "Two", "Three"], "itemT": [0.5, 1.0, 1.5], "sub": "sub"},
        {"type": "terminal", "dur": 4.0, "lines": ["$ rsepuh pv", "done"]},
        {"type": "stat", "dur": 3.0, "value": 10, "suffix": "s", "label": "fast"},
        {"type": "caption", "dur": 3.0, "text": "Say the point in one breath"},
        {"type": "cta", "dur": 3.0, "title": "rsepuh", "lines": ["tagline"], "button": "Try it", "url": "example.com"},
        {"type": "media", "dur": 3.0, "caption": "Motion + 3D"},
        {"type": "code", "dur": 8.0, "code": ["useEffect(() => {", "  go(`x ${y}`);", "}, [y]);"],
         "focus": [{"line": 1, "atBeat": 2, "untilBeat": 6, "label": "runs after render"}]}
    ]);
    let d = project("all");
    let (ok, log, out) = run(&d, &plan(scenes.clone()));
    assert!(ok, "{log}");
    let manifest: Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json")).unwrap()).unwrap();
    let list = manifest.as_array().unwrap();
    assert_eq!(list.len(), scenes.as_array().unwrap().len());
    for (i, m) in list.iter().enumerate() {
        let doc: Value =
            serde_json::from_slice(&fs::read(out.join(format!("scene-{i:02}.json"))).unwrap())
                .unwrap();
        assert_eq!(
            doc["nm"],
            format!("rs-t-s{i:02}"),
            "deterministic comp name the compositor looks up"
        );
        assert_eq!(
            (doc["w"].as_u64(), doc["h"].as_u64(), doc["fr"].as_u64()),
            (Some(1080), Some(1920), Some(30))
        );
        let want = (scenes[i]["dur"].as_f64().unwrap() * 30.0).round();
        assert_eq!(
            doc["op"].as_f64().unwrap(),
            want,
            "scene {i} lasts exactly its duration"
        );
        assert_eq!(m["op"].as_f64().unwrap(), want);
        if scenes[i]["type"] != "media" {
            assert!(
                !doc["layers"].as_array().unwrap().is_empty(),
                "scene {i} has motion"
            );
        }
        assert!(
            doc.get("slots").is_none(),
            "flat files carry no slots: EffectCraft imports plain values"
        );
    }
}

#[test]
fn custom_writers_and_lottie_files_are_picked_up_and_bad_ones_fail_loudly() {
    if !have_python() {
        return;
    }
    let d = project("custom");
    fs::create_dir_all(d.join("lottie/scenes")).unwrap();
    fs::write(
        d.join("lottie/scenes/dot.py"),
        "from rs import *\ndef build(doc, sc, ctx):\n    doc.add(Layer([group([ellipse(100), fill('#ff0000')])], pos=(540, 960)))\n",
    )
    .unwrap();
    fs::create_dir_all(d.join("assets")).unwrap();
    fs::write(
        d.join("assets/mine.json"),
        json!({"v": "5.7.0", "fr": 24, "ip": 0, "op": 48, "w": 400, "h": 400, "nm": "authored elsewhere", "layers": []}).to_string(),
    )
    .unwrap();
    let scenes = json!([
        {"type": "custom", "dur": 1.0, "file": "lottie/scenes/dot.py"},
        {"type": "lottie", "dur": 2.0, "file": "assets/mine.json"}
    ]);
    let (ok, log, out) = run(&d, &plan(scenes));
    assert!(ok, "{log}");
    let dot: Value = serde_json::from_slice(&fs::read(out.join("scene-00.json")).unwrap()).unwrap();
    assert_eq!(dot["layers"].as_array().unwrap().len(), 1);
    let lot: Value = serde_json::from_slice(&fs::read(out.join("scene-01.json")).unwrap()).unwrap();
    assert_eq!(
        (lot["nm"].as_str(), lot["w"].as_u64()),
        (Some("rs-t-s01"), Some(400)),
        "renamed, size kept"
    );
    // a writer without build() is an error that names the file
    fs::write(d.join("lottie/scenes/bad.py"), "x = 1\n").unwrap();
    let (ok, log, _) = run(
        &d,
        &plan(json!([{"type": "custom", "dur": 1.0, "file": "lottie/scenes/bad.py"}])),
    );
    assert!(!ok && log.contains("must define build"), "{log}");
}

#[test]
fn kinetic_text_is_vector_glyphs_with_per_letter_motion() {
    if !have_python() {
        return;
    }
    let d = project("text");
    let code = "import sys, json\nsys.path.insert(0, 'lottie')\nfrom kit import *\nd = Doc('x', 800, 300, seconds=2)\n\
d.add(text('Hi [[you]]', size=100, x=400, y=150, preset='drop', stagger=0.05, accent='#ff0000'))\n\
print(json.dumps(d.to_json()))\n";
    let o = Command::new("python3")
        .current_dir(&d)
        .args(["-c", code])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let doc: Value = serde_json::from_slice(&o.stdout).unwrap();
    let groups = doc["layers"][0]["shapes"].as_array().unwrap();
    assert_eq!(groups.len(), 5, "one group per visible letter: H i y o u");
    let animated = groups
        .iter()
        .filter(|g| g["it"].as_array().unwrap().last().unwrap()["p"]["a"] == 1)
        .count();
    assert_eq!(animated, 5, "every letter has its own position animation");
    assert!(
        doc.to_string().contains("[1.0,0.0,0.0,1]"),
        "[[ ]] words use the accent colour"
    );
}
