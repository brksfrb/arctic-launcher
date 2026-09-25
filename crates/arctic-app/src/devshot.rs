//! Debug builds only: `ARCTIC_DEVSHOT=out.png` saves a screenshot of the
//! window after `ARCTIC_DEVSHOT_DELAY` seconds (default 4) and closes the
//! app. Lets UI changes be checked without a visible desktop.

use std::path::PathBuf;

use eframe::egui;

#[derive(Default)]
pub struct DevShot {
    path: Option<PathBuf>,
    delay: f64,
    requested: bool,
}

impl DevShot {
    pub fn from_env() -> Self {
        if !cfg!(debug_assertions) {
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
        }
    }

    pub fn update(&mut self, ctx: &egui::Context) {
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
            let [w, h] = image.size;
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes)
                && let Err(e) = buf.save(path)
            {
                log::error!("devshot: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
