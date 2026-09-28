//! Background jobs for the Screenshots tab: listing, thumbnails (cached on
//! disk by path and time) and copying an image.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::tasks::{Event, Tasks};

const THUMB_W: u32 = 480;

impl Tasks {
    pub fn screenshot_list(&self) {
        self.run(|t| {
            t.send(Event::Screenshots(arctic_core::screenshots::list(t.dirs())));
        });
    }

    pub fn screenshot_thumb(&self, path: PathBuf) {
        self.run(move |t| {
            let result = thumb(&path, &t.dirs().cache().join("thumbs"));
            t.send(Event::ScreenshotThumb(path, result));
        });
    }

    pub fn screenshot_copy(&self, path: PathBuf) {
        self.run(move |t| {
            let result = image::open(&path)
                .map(|img| {
                    let rgba = img.to_rgba8();
                    egui::ColorImage::from_rgba_unmultiplied(
                        [rgba.width() as usize, rgba.height() as usize],
                        rgba.as_raw(),
                    )
                })
                .map_err(|e| e.to_string());
            t.send(Event::ScreenshotCopied(result));
        });
    }
}

/// A small PNG of the screenshot, from the cache when it's still current.
fn thumb(path: &Path, cache: &Path) -> Result<Vec<u8>, String> {
    use std::hash::{Hash, Hasher};
    let modified = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map_err(|e| e.to_string())?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    modified.hash(&mut h);
    let cached = cache.join(format!("{:016x}.png", h.finish()));
    if let Ok(bytes) = std::fs::read(&cached) {
        return Ok(bytes);
    }
    let img = image::open(path).map_err(|e| e.to_string())?;
    let small = img.thumbnail(THUMB_W, THUMB_W);
    let mut out = std::io::Cursor::new(Vec::new());
    small
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let bytes = out.into_inner();
    let _ = std::fs::create_dir_all(cache);
    let _ = std::fs::write(&cached, &bytes);
    Ok(bytes)
}
