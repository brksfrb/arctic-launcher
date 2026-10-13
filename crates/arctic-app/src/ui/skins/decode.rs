//! Capes decoded off the UI thread. HD animated capes are tall stacks of frames (up to
//! 1024×12288): decoding a page of them at once used to stall the window when the Capes
//! section opened. Workers decode; the UI uploads a few finished ones each frame.

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};

use eframe::egui::{self, ColorImage};

/// Workers decoding at once.
const WORKERS: usize = 3;
/// Pixels uploaded to the GPU per frame at most (a big animated cape is ~16 million).
const UPLOAD_BUDGET: usize = 24 << 20;

struct Job {
    key: String,
    png: Vec<u8>,
    max_width: u32,
    still: bool,
}

/// A decoded cape: its frames top to bottom (None: not a cape image).
pub(super) struct Decoded {
    pub key: String,
    pub frames: Option<Vec<ColorImage>>,
}

pub(super) struct Decoder {
    jobs: Option<Sender<Job>>,
    done_tx: Sender<Decoded>,
    done: Receiver<Decoded>,
    pending: HashSet<String>,
}

impl Default for Decoder {
    fn default() -> Self {
        let (done_tx, done) = mpsc::channel();
        Self {
            jobs: None,
            done_tx,
            done,
            pending: HashSet::new(),
        }
    }
}

impl Decoder {
    pub fn is_pending(&self, key: &str) -> bool {
        self.pending.contains(key)
    }

    /// Decode `png` in the background (once per key until it's finished), frames at most
    /// `max_width` wide; `still`: the first frame only.
    pub fn request(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        png: Vec<u8>,
        max_width: u32,
        still: bool,
    ) {
        if !self.pending.insert(key.to_owned()) {
            return;
        }
        let jobs = self
            .jobs
            .get_or_insert_with(|| spawn(ctx, self.done_tx.clone()));
        let job = Job {
            key: key.to_owned(),
            png,
            max_width,
            still,
        };
        if jobs.send(job).is_err() {
            log::warn!("cape decoder stopped");
        }
    }

    /// Finished capes, as many as fit this frame's upload budget.
    pub fn finished(&mut self, ctx: &egui::Context) -> Vec<Decoded> {
        let mut out = Vec::new();
        let mut pixels = 0;
        while pixels < UPLOAD_BUDGET {
            let Ok(d) = self.done.try_recv() else {
                break;
            };
            pixels += d
                .frames
                .iter()
                .flatten()
                .map(|f| f.width() * f.height())
                .sum::<usize>();
            self.pending.remove(&d.key);
            out.push(d);
        }
        if !self.pending.is_empty() {
            // More are coming (or waiting for next frame's budget).
            ctx.request_repaint();
        }
        out
    }
}

fn spawn(ctx: &egui::Context, done: Sender<Decoded>) -> Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    let rx = Arc::new(Mutex::new(rx));
    for i in 0..WORKERS {
        let (rx, done, ctx) = (rx.clone(), done.clone(), ctx.clone());
        let spawned = std::thread::Builder::new()
            .name(format!("cape-decode-{i}"))
            .spawn(move || {
                loop {
                    let job = rx.lock().unwrap_or_else(PoisonError::into_inner).recv();
                    let Ok(job) = job else {
                        return;
                    };
                    let frames = cape_frames(&job.png, job.max_width).map(|mut frames| {
                        if job.still {
                            frames.truncate(1);
                        }
                        frames
                    });
                    if done
                        .send(Decoded {
                            key: job.key,
                            frames,
                        })
                        .is_err()
                    {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        if let Err(e) = spawned {
            log::warn!("cape decoder thread: {e}");
        }
    }
    tx
}

/// A cape image's frames (one, or a stack of animation frames), scaled down to `max_width` if
/// wider: a card shows a cape a hundred pixels tall, and every frame of a 1024-wide animated
/// cape is 2 MB of video memory.
pub(super) fn cape_frames(png: &[u8], max_width: u32) -> Option<Vec<ColorImage>> {
    // Only cape-sized images get decoded (a bad file could claim any size).
    if !arctic_core::cosmetics::is_cape_png(png) {
        return None;
    }
    let image = image::load_from_memory(png).ok()?.to_rgba8();
    let (w, h) = image.dimensions();
    let count = arctic_core::cosmetics::cape_frames(w, h).unwrap_or(1);
    let frame_h = h / count;
    let row = w as usize * 4;
    let raw = image.as_raw();
    Some(
        (0..count as usize)
            .map(|i| {
                let start = i * frame_h as usize * row;
                let bytes = &raw[start..start + frame_h as usize * row];
                if w <= max_width {
                    return ColorImage::from_rgba_unmultiplied(
                        [w as usize, frame_h as usize],
                        bytes,
                    );
                }
                let (sw, sh) = (max_width, (frame_h * max_width / w).max(1));
                match image::RgbaImage::from_raw(w, frame_h, bytes.to_vec()) {
                    Some(frame) => {
                        let small = image::imageops::resize(
                            &frame,
                            sw,
                            sh,
                            image::imageops::FilterType::Triangle,
                        );
                        ColorImage::from_rgba_unmultiplied(
                            [sw as usize, sh as usize],
                            small.as_raw(),
                        )
                    }
                    None => {
                        ColorImage::from_rgba_unmultiplied([w as usize, frame_h as usize], bytes)
                    }
                }
            })
            .collect(),
    )
}
