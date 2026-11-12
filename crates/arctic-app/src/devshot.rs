//! Debug and developer (`ARCTIC_DEV_PACK`) builds only:
//! `ARCTIC_DEVSHOT=out.png` saves a screenshot of the window after
//! `ARCTIC_DEVSHOT_DELAY` seconds (default 4) and closes the app. Lets UI
//! changes be checked without a visible desktop.
//! `ARCTIC_DEVSHOT_TIP=<index>` pins a sidebar tip for pointer-click navigation proofs.

use std::path::PathBuf;

use eframe::egui;

/// Debug builds and developer builds (made with `ARCTIC_DEV_PACK`).
pub fn enabled() -> bool {
    cfg!(debug_assertions) || option_env!("ARCTIC_DEV_PACK").is_some()
}

#[derive(Default)]
pub struct DevShot {
    path: Option<PathBuf>,
    delay: f64,
    requested: bool,
    /// `ARCTIC_DEVSHOT_INSTANCE`: open this instance's page first.
    pub open_instance: Option<String>,
    /// `ARCTIC_DEVSHOT_TAB`: open this tab first (its position in the sidebar, from 0).
    pub open_tab: Option<usize>,
    /// `ARCTIC_DEVSHOT_JOIN`: start the selected instance into this server.
    pub join: Option<String>,
    /// `ARCTIC_DEVSHOT_LAUNCH`: start the selected instance (`duel`: into a duel).
    pub launch: Option<String>,
    /// `ARCTIC_DEVSHOT_FRAMES=<dir>`: once the screenshot viewer is opened by `ARCTIC_DEVSHOT_OPEN_SHOT`,
    /// save a picture of every frame (`frame_000.png`...) and a `frames.log` of the viewer state.
    frames_dir: Option<PathBuf>,
    recording: bool,
    requested_frames: u32,
    saved_frames: u32,
    pending_saves: Vec<std::thread::JoinHandle<()>>,
    frames_total: u32,
}

impl DevShot {
    pub fn from_env() -> Self {
        if !enabled() {
            return Self::default();
        }
        let path = std::env::var_os("ARCTIC_DEVSHOT").map(PathBuf::from);
        let delay = std::env::var("ARCTIC_DEVSHOT_DELAY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4.0);
        Self {
            path,
            delay,
            requested: false,
            open_instance: std::env::var("ARCTIC_DEVSHOT_INSTANCE").ok(),
            open_tab: std::env::var("ARCTIC_DEVSHOT_TAB")
                .ok()
                .and_then(|v| v.parse().ok()),
            join: std::env::var("ARCTIC_DEVSHOT_JOIN").ok(),
            launch: std::env::var("ARCTIC_DEVSHOT_LAUNCH").ok(),
            frames_dir: std::env::var_os("ARCTIC_DEVSHOT_FRAMES").map(PathBuf::from),
            frames_total: std::env::var("ARCTIC_DEVSHOT_FRAME_COUNT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            ..Self::default()
        }
    }

    /// Whether `ARCTIC_DEVSHOT_FRAMES` asks for a per-frame recording.
    pub fn wants_frames(&self) -> bool {
        self.frames_dir.is_some()
    }

    pub fn start_recording(&mut self) {
        if let Some(dir) = &self.frames_dir {
            let _ = std::fs::create_dir_all(dir);
            self.recording = true;
        }
    }

    /// Appends one line to `frames.log` (what the viewer was doing this frame).
    pub fn frame_note(&self, line: &str) {
        use std::io::Write;
        if let (true, Some(dir)) = (self.recording, &self.frames_dir)
            && let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("frames.log"))
        {
            let _ = writeln!(f, "{line}");
        }
    }

    /// `ARCTIC_DEVSHOT_SLOWMO=<n>`: animations take n times as long, so the recording (which can't
    /// keep 60 frames a second) still catches every step of them.
    pub fn slowmo() -> f64 {
        if !enabled() {
            return 1.0;
        }
        std::env::var("ARCTIC_DEVSHOT_SLOWMO")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0)
    }

    fn record_frames(&mut self, ctx: &egui::Context) {
        if self.recording {
            let t = (1.0 / 12.0 * Self::slowmo()) as f32;
            ctx.all_styles_mut(|s| s.animation_time = t);
        }
        let Some(dir) = self.frames_dir.clone().filter(|_| self.recording) else {
            return;
        };
        ctx.request_repaint();
        let images: Vec<_> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
                .collect()
        });
        for image in images {
            let [w, h] = image.size;
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            // Encoded off-thread so the frames being recorded keep their real pace.
            let path = dir.join(format!("frame_{:03}.png", self.saved_frames));
            self.pending_saves.push(std::thread::spawn(move || {
                if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                    let _ = buf.save(path);
                }
            }));
            self.saved_frames += 1;
        }
        if self.requested_frames < self.frames_total {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.frame_note(&format!(
                "request {} t={:.3}",
                self.requested_frames,
                ctx.input(|i| i.time)
            ));
            self.requested_frames += 1;
        } else if self.saved_frames >= self.frames_total {
            for t in self.pending_saves.drain(..) {
                let _ = t.join();
            }
            crate::tray::set_quitting();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub fn update(&mut self, ctx: &egui::Context) {
        self.record_frames(ctx);
        let Some(path) = &self.path else {
            return;
        };
        ctx.request_repaint();
        if !self.requested && ctx.input(|i| i.time) >= self.delay {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.requested = true;
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            if std::env::var_os("ARCTIC_DEVSHOT_MEMORY").is_some() {
                eprintln!("[memory]\n{}", crate::memstats::report(ctx));
                if std::env::var_os("ARCTIC_DEVSHOT_TRIM").is_some() {
                    crate::trim::now(ctx);
                    eprintln!("[memory after trim]\n{}", crate::memstats::report(ctx));
                }
            }
            let [w, h] = image.size;
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes)
                && let Err(e) = buf.save(path)
            {
                log::error!("devshot: {e}");
            }
            crate::tray::set_quitting();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
