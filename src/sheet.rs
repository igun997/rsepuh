//! Contact sheet: tile the preview frames of a render into one PNG so a human or an agent can
//! judge the whole piece at a glance.

use std::fs;
use std::path::Path;

use crate::Result;

fn decode_rgb(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(fs::read(path)?));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut r = dec.read_info()?;
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf)?;
    let ch = info.color_type.samples();
    let px = &buf[..info.buffer_size()];
    let mut rgb = Vec::with_capacity((info.width * info.height * 3) as usize);
    for p in px.chunks_exact(ch) {
        if ch >= 3 {
            rgb.extend_from_slice(&p[..3]);
        } else {
            rgb.extend_from_slice(&[p[0], p[0], p[0]]);
        }
    }
    Ok((info.width, info.height, rgb))
}

/// Tile every `*.png` in `frames` (name order) into `out`, `cols` per row. Returns the frame count.
pub fn make(frames: &Path, out: &Path, cols: usize) -> Result<usize> {
    let mut files: Vec<_> = fs::read_dir(frames)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err("no frames to tile".into());
    }
    let imgs: Vec<_> = files.iter().map(|f| decode_rgb(f)).collect::<Result<_>>()?;
    let (cw, ch) = (imgs[0].0 as usize, imgs[0].1 as usize);
    let cols = cols.clamp(1, imgs.len());
    let rows = imgs.len().div_ceil(cols);
    let gap = 4usize;
    let (sw, sh) = (cols * cw + (cols + 1) * gap, rows * ch + (rows + 1) * gap);
    let mut sheet = vec![40u8; sw * sh * 3];
    for (i, (w, h, rgb)) in imgs.iter().enumerate() {
        let (w, h) = (*w as usize, *h as usize);
        let (ox, oy) = (gap + (i % cols) * (cw + gap), gap + (i / cols) * (ch + gap));
        for y in 0..h.min(ch) {
            let dst = ((oy + y) * sw + ox) * 3;
            let n = w.min(cw) * 3;
            sheet[dst..dst + n].copy_from_slice(&rgb[y * w * 3..y * w * 3 + n]);
        }
    }
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(fs::File::create(out)?),
        sw as u32,
        sh as u32,
    );
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&sheet)?;
    Ok(imgs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(p: &Path, w: u32, h: u32, v: u8) {
        let mut enc = png::Encoder::new(fs::File::create(p).unwrap(), w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&vec![v; (w * h * 3) as usize])
            .unwrap();
    }

    #[test]
    fn tiles_frames_into_a_grid_with_gaps() {
        let d = std::env::temp_dir().join(format!("rsepuh-sheet-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        for (i, v) in [10u8, 100, 200].iter().enumerate() {
            write_png(&d.join(format!("f{i:02}.png")), 8, 6, *v);
        }
        let out = d.join("sheet.png");
        assert_eq!(make(&d, &out, 2).unwrap(), 3);
        let (w, h, rgb) = decode_rgb(&out).unwrap();
        assert_eq!((w, h), (2 * 8 + 3 * 4, 2 * 6 + 3 * 4));
        // second frame, first pixel: row gap + col offset
        let at = ((4 * w as usize) + 4 + 8 + 4) * 3;
        assert_eq!(rgb[at], 100);
        assert!(make(&d.join("nothing"), &out, 2).is_err());
    }
}
