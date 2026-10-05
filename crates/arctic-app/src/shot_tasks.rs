//! Background jobs for the Screenshots tab: listing, thumbnails (cached on
//! disk by path and time) and copying an image.

use std::path::{Path, PathBuf};


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
            let result = copy_image(&path);
            t.send(Event::ScreenshotCopied(result));
        });
    }
}

/// How often the clipboard is tried when another program has it open
/// (clipboard managers and overlays do, for a moment).
const CLIPBOARD_TRIES: u32 = 8;

/// Put the picture on the clipboard, trying again while it's busy.
fn copy_image(path: &Path) -> Result<(), String> {
    let rgba = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let mut last = String::new();
    for attempt in 0..CLIPBOARD_TRIES {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(120));
        }
        let data = arboard::ImageData {
            width,
            height,
            bytes: std::borrow::Cow::Borrowed(rgba.as_raw()),
        };
        match arboard::Clipboard::new().and_then(|mut c| c.set_image(data)) {
            Ok(()) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("the clipboard is busy ({last})"))
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
