//! Spritesheet slicing + pet-pack manifest + per-pet state→clip bindings. Ports
//! `SpriteSlicer.swift` and `PetBindings.swift`.
//!
//! Slicing detects the transparent gutters between cells, so no grid metadata
//! is required: one non-empty row band = one animation clip, each non-empty
//! column band within it = one frame.

use crate::state::PetMood;
use image::RgbaImage;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// `pet.json` — the Petdex/Codex pet-pack manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PetManifest {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "spritesheetPath")]
    pub spritesheet_path: String,
}

impl PetManifest {
    pub fn decode(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }
}

/// Slices a spritesheet into clips (one per sheet row) using alpha-gutter
/// detection. Empty rows/cells are skipped. `alpha_threshold` defaults to 16.
pub fn slice(image: &RgbaImage, alpha_threshold: u8) -> Vec<Vec<RgbaImage>> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let data = image.as_raw(); // RGBA8, row-major

    let mut col_has = vec![false; w];
    let mut row_has = vec![false; h];
    for y in 0..h {
        let row_start = y * w * 4;
        let mut row_any = false;
        for x in 0..w {
            if data[row_start + x * 4 + 3] > alpha_threshold {
                col_has[x] = true;
                row_any = true;
            }
        }
        if row_any {
            row_has[y] = true;
        }
    }

    let col_bands = segments(&col_has);
    let row_bands = segments(&row_has);
    if col_bands.is_empty() || row_bands.is_empty() {
        return Vec::new();
    }

    let mut clips: Vec<Vec<RgbaImage>> = Vec::new();
    for &(ry0, ry1) in &row_bands {
        let mut clip: Vec<RgbaImage> = Vec::new();
        for &(cx0, cx1) in &col_bands {
            if cell_has_content(data, w, cx0, cx1, ry0, ry1, alpha_threshold) {
                clip.push(crop(image, cx0, ry0, cx1 - cx0, ry1 - ry0));
            }
        }
        if !clip.is_empty() {
            clips.push(clip);
        }
    }
    clips
}

/// Contiguous `true` bands as `(lower, upper)` half-open ranges.
fn segments(occupancy: &[bool]) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut start: Option<usize> = None;
    for (i, &filled) in occupancy.iter().enumerate() {
        match (filled, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                result.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        result.push((s, occupancy.len()));
    }
    result
}

fn cell_has_content(
    data: &[u8],
    width: usize,
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    threshold: u8,
) -> bool {
    for y in y0..y1 {
        let row_start = y * width * 4;
        for x in x0..x1 {
            if data[row_start + x * 4 + 3] > threshold {
                return true;
            }
        }
    }
    false
}

fn crop(image: &RgbaImage, x: usize, y: usize, w: usize, h: usize) -> RgbaImage {
    let mut out = RgbaImage::new(w as u32, h as u32);
    for dy in 0..h {
        for dx in 0..w {
            out.put_pixel(
                dx as u32,
                dy as u32,
                *image.get_pixel((x + dx) as u32, (y + dy) as u32),
            );
        }
    }
    out
}

/// A loaded pet pack: its manifest plus the sliced animation clips. Mirrors the
/// macOS `ImagePetPack` (one clip per sheet row, each a list of frames).
#[derive(Debug, Clone)]
pub struct PetPack {
    pub manifest: PetManifest,
    pub clips: Vec<Vec<RgbaImage>>,
    pub dir: PathBuf,
}

impl PetPack {
    pub fn clip_count(&self) -> usize {
        self.clips.len()
    }

    /// Frames of the clip at `index`, clamped into range.
    pub fn clip(&self, index: usize) -> &[RgbaImage] {
        if self.clips.is_empty() {
            return &[];
        }
        &self.clips[index.min(self.clips.len() - 1)]
    }
}

/// Loads a pet pack directory (`pet.json` + spritesheet) and slices it. Mirrors
/// `SpriteSlicer.loadPack`. Returns `None` if the manifest/sheet is missing or
/// nothing sliced.
pub fn load_pack(dir: &Path) -> Option<PetPack> {
    let manifest = PetManifest::decode(&std::fs::read(dir.join("pet.json")).ok()?)?;
    let sheet = image::open(dir.join(&manifest.spritesheet_path)).ok()?.to_rgba8();
    let clips = slice(&sheet, 16);
    if clips.is_empty() {
        return None;
    }
    Some(PetPack { manifest, clips, dir: dir.to_path_buf() })
}

/// Maps each pet mood to a clip index of an imported sprite pet. Ports
/// `PetBindings.swift`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetBindings {
    pub by_mood: HashMap<String, usize>,
}

impl PetBindings {
    pub fn clip_index(&self, mood: PetMood) -> usize {
        self.by_mood.get(mood.raw()).copied().unwrap_or(0)
    }

    /// Spreads the first clips across moods (idle, working, waiting, done,
    /// celebrate), clamped to what the pack has.
    pub fn defaults(clip_count: usize) -> Self {
        let mut by_mood = HashMap::new();
        for (i, mood) in PetMood::ALL.iter().enumerate() {
            let idx = if clip_count > 0 { i.min(clip_count - 1) } else { 0 };
            by_mood.insert(mood.raw().to_string(), idx);
        }
        PetBindings { by_mood }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    /// Builds a sheet of `rows`×`cols` opaque blocks separated by 2px
    /// transparent gutters (and a 2px transparent border).
    fn grid_sheet(rows: usize, cols: usize, cell: usize, gutter: usize) -> RgbaImage {
        let w = gutter + cols * (cell + gutter);
        let h = gutter + rows * (cell + gutter);
        let mut img = RgbaImage::new(w as u32, h as u32); // all transparent
        for r in 0..rows {
            for c in 0..cols {
                let x0 = gutter + c * (cell + gutter);
                let y0 = gutter + r * (cell + gutter);
                for y in y0..y0 + cell {
                    for x in x0..x0 + cell {
                        img.put_pixel(x as u32, y as u32, Rgba([255, 0, 0, 255]));
                    }
                }
            }
        }
        img
    }

    #[test]
    fn slices_rows_into_clips_and_columns_into_frames() {
        let sheet = grid_sheet(3, 4, 8, 2);
        let clips = slice(&sheet, 16);
        assert_eq!(clips.len(), 3, "one clip per row band");
        for clip in &clips {
            assert_eq!(clip.len(), 4, "one frame per column band");
            for frame in clip {
                assert_eq!((frame.width(), frame.height()), (8, 8));
            }
        }
    }

    #[test]
    fn empty_image_yields_no_clips() {
        let blank = RgbaImage::new(16, 16); // fully transparent
        assert!(slice(&blank, 16).is_empty());
    }

    #[test]
    fn bindings_default_spread_and_clamp() {
        let b = PetBindings::defaults(5);
        assert_eq!(b.clip_index(PetMood::Idle), 0);
        assert_eq!(b.clip_index(PetMood::Working), 1);
        assert_eq!(b.clip_index(PetMood::Celebrate), 4);

        let few = PetBindings::defaults(2);
        assert_eq!(few.clip_index(PetMood::Idle), 0);
        assert_eq!(few.clip_index(PetMood::Working), 1);
        assert_eq!(few.clip_index(PetMood::Celebrate), 1, "clamped to last clip");

        let none = PetBindings::defaults(0);
        assert_eq!(none.clip_index(PetMood::Working), 0);
    }

    #[test]
    fn manifest_decodes() {
        let json = br#"{"id":"boba","displayName":"Boba","spritesheetPath":"sheet.png"}"#;
        let m = PetManifest::decode(json).unwrap();
        assert_eq!(m.id, "boba");
        assert_eq!(m.display_name, "Boba");
        assert_eq!(m.spritesheet_path, "sheet.png");
        assert_eq!(m.description, None);
    }

    #[test]
    fn load_pack_reads_manifest_and_slices_sheet() {
        let dir = tempfile::tempdir().unwrap();
        grid_sheet(2, 3, 8, 2).save(dir.path().join("sheet.png")).unwrap();
        std::fs::write(
            dir.path().join("pet.json"),
            br#"{"id":"boba","displayName":"Boba","spritesheetPath":"sheet.png"}"#,
        )
        .unwrap();

        let pack = load_pack(dir.path()).expect("pack loads");
        assert_eq!(pack.manifest.id, "boba");
        assert_eq!(pack.clip_count(), 2, "2 row bands → 2 clips");
        assert_eq!(pack.clip(0).len(), 3, "3 col bands → 3 frames");
    }

    #[test]
    fn load_pack_missing_manifest_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_pack(dir.path()).is_none());
    }
}
