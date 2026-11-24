//! rsepuh: "Edit layaknya sepuh". UGC-style motion videos built with EffectCraft, Blender and
//! optional Gemini stills, rendered on remote sandboxes through rcargo.

pub mod assets;
pub mod audio;
pub mod cmds;
pub mod gemini;
pub mod mcp;
pub mod remote;
pub mod sheet;
pub mod spec;
pub mod subs;
pub mod synth;
pub mod util;
pub mod verify;
pub mod voice;

pub type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;
