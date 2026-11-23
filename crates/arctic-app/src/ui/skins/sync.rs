use std::path::PathBuf;
use std::time::{Duration, Instant};

use arctic_core::cosmetics::{Look, texture_hash};
use arctic_core::skins::Variant;
use eframe::egui;

use crate::app::ArcticApp;
use crate::skin_tasks::{SkinChange, SyncedMinecraftSkin};
use crate::t;
use crate::toasts::Kind;

const DEBOUNCE: Duration = Duration::from_millis(500);

struct PendingSkin {
    account: String,
    hash: String,
    png: Vec<u8>,
    variant: Variant,
    due: Instant,
    saved: bool,
}

#[derive(Default)]
pub(super) struct SkinSync {
    pending: Option<PendingSkin>,
    notice: Option<(String, SkinChange)>,
}

impl SkinSync {
    pub(super) fn equip(&mut self, account: &str, png: &[u8], variant: Variant, now: Instant) {
        self.pending = Some(PendingSkin {
            account: account.to_owned(),
            hash: texture_hash(png),
            png: png.to_vec(),
            variant,
            due: now + DEBOUNCE,
            saved: false,
        });
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    pub(super) fn dismiss_notice(&mut self) {
        self.notice = None;
    }

    pub(super) fn saved(&mut self, account: &str, look: Option<&Look>) {
        let Some(pending) = self.pending.as_mut().filter(|skin| skin.account == account) else {
            return;
        };
        match look {
            Some(look) => {
                pending.saved = look.skin.as_deref() == Some(&pending.hash)
                    && look.variant() == pending.variant;
            }
            None if !pending.saved => self.cancel(),
            None => {}
        }
    }

    fn take_ready(&mut self, now: Instant) -> Option<PendingSkin> {
        if self
            .pending
            .as_ref()
            .is_some_and(|skin| skin.saved && now >= skin.due)
        {
            self.pending.take()
        } else {
            None
        }
    }
}

impl ArcticApp {
    pub(crate) fn tick_minecraft_skin_sync(&mut self, ctx: &egui::Context) {
        let Some(pending) = self.skins.skin_sync.pending.as_ref() else {
            return;
        };
        if self.skins.loaded_for.as_ref() != Some(&self.skins_dir()) {
            self.skins.skin_sync.cancel();
            return;
        }
        let account = self.accounts.active().filter(|account| {
            self.settings.sync_minecraft_skin
                && account.is_microsoft()
                && account.id == pending.account
        });
        let Some(account) = account else {
            self.skins.skin_sync.cancel();
            return;
        };
        if !pending.saved || self.skins.busy || self.skins.arctic_busy {
            return;
        }
        let now = Instant::now();
        if let Some(skin) = self.skins.skin_sync.take_ready(now) {
            self.skins.busy = true;
            self.tasks.sync_minecraft_skin(
                account.clone(),
                skin.variant,
                skin.png,
                !self.settings.minecraft_skin_sync_explained,
            );
        } else if let Some(pending) = &self.skins.skin_sync.pending {
            ctx.request_repaint_after(pending.due.saturating_duration_since(now));
        }
    }

    pub(super) fn on_minecraft_skin_synced(
        &mut self,
        root: PathBuf,
        account: String,
        result: Result<SyncedMinecraftSkin, String>,
    ) {
        if root != self.dirs.profile_root() {
            return;
        }
        self.skins.busy = false;
        match result {
            Ok(skin) => {
                self.skins.textures.remove("mc");
                self.skins.failed.remove("mc");
                if let Some(Ok(current)) = self.skins.account.get_mut(&account) {
                    current.profile = skin.profile;
                    current.skin_png = Some(skin.png);
                } else {
                    // Let the normal profile load retrieve the capes as well.
                    self.skins.account.remove(&account);
                }
                if let Some(undo) = skin.undo {
                    self.skins.skin_sync.notice = Some((account, undo));
                    self.settings.minecraft_skin_sync_explained = true;
                    self.persist_settings();
                }
            }
            Err(error) => {
                self.toasts
                    .push(Kind::Error, t!("skins.minecraft_skin_change_failed"), error)
            }
        }
    }

    pub(super) fn minecraft_skin_sync_controls(&mut self, ui: &mut egui::Ui) {
        if !self.accounts.active().is_some_and(|a| a.is_microsoft()) {
            return;
        }
        let setting = ui.checkbox(
            &mut self.settings.sync_minecraft_skin,
            t!("skins.sync_minecraft_skin"),
        );
        if setting.changed() {
            if !self.settings.sync_minecraft_skin {
                self.skins.skin_sync.cancel();
            }
            self.persist_settings();
        }
        if !self.skins.skin_sync.notice.as_ref().is_some_and(|(id, _)| {
            self.accounts
                .active()
                .is_some_and(|active| &active.id == id)
        }) {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(t!("skins.minecraft_skin_synced"));
            if ui
                .add_enabled(!self.skins.busy, egui::Link::new(t!("settings.undo")))
                .clicked()
                && let Some((_, undo)) = self.skins.skin_sync.notice.take()
            {
                self.change_minecraft_skin(undo);
            }
            if ui.link(t!("settings.settings")).clicked() {
                setting.scroll_to_me(Some(egui::Align::Center));
                setting.request_focus();
            }
            if ui.link(t!("nav.dismiss")).clicked() {
                self.skins.skin_sync.dismiss_notice();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(png: &[u8], variant: Variant) -> Look {
        Look {
            skin: Some(texture_hash(png)),
            model: variant.api_name().to_owned(),
            ..Look::default()
        }
    }

    #[test]
    fn rapid_equips_only_upload_the_last_saved_pick() {
        let mut sync = SkinSync::default();
        let now = Instant::now();
        let second_pick = now + Duration::from_millis(100);
        sync.equip("player", b"first", Variant::Classic, now);
        sync.saved("player", Some(&look(b"first", Variant::Classic)));
        sync.equip("player", b"second", Variant::Slim, second_pick);
        assert!(sync.take_ready(second_pick + DEBOUNCE).is_none());
        sync.saved("player", Some(&look(b"second", Variant::Slim)));
        assert!(sync.take_ready(now + DEBOUNCE).is_none());
        let upload = sync.take_ready(second_pick + DEBOUNCE).unwrap();
        assert_eq!(upload.png, b"second");
        assert_eq!(upload.variant, Variant::Slim);
        assert!(sync.take_ready(second_pick + DEBOUNCE).is_none());
    }

    #[test]
    fn preview_and_unrelated_saves_do_not_enqueue_uploads() {
        let mut skins = super::super::SkinsUi::default();
        let now = Instant::now();
        skins.hover_skin("lib:preview", Variant::Slim);
        assert_eq!(
            skins.hover.take(),
            Some(super::super::Hover::Skin(
                "lib:preview".into(),
                Variant::Slim
            ))
        );
        // Changing a cape can save a look that already has an equipped skin.
        skins
            .skin_sync
            .saved("player", Some(&look(b"already worn", Variant::Classic)));
        assert!(skins.skin_sync.take_ready(now + DEBOUNCE).is_none());
    }

    #[test]
    fn confirmation_requires_the_selected_variant_and_account() {
        let mut sync = SkinSync::default();
        let now = Instant::now();
        sync.equip("player", b"same pixels", Variant::Slim, now);
        sync.saved("player", Some(&look(b"same pixels", Variant::Classic)));
        sync.saved("other", Some(&look(b"same pixels", Variant::Slim)));
        assert!(sync.take_ready(now + DEBOUNCE).is_none());
        sync.saved("player", Some(&look(b"same pixels", Variant::Slim)));
        assert_eq!(
            sync.take_ready(now + DEBOUNCE).unwrap().variant,
            Variant::Slim
        );
    }

    #[test]
    fn failed_save_or_taking_skin_off_cancels_upload() {
        let mut sync = SkinSync::default();
        let now = Instant::now();
        sync.equip("player", b"skin", Variant::Classic, now);
        sync.saved("player", None);
        sync.saved("player", Some(&look(b"skin", Variant::Classic)));
        assert!(sync.take_ready(now + DEBOUNCE).is_none());
        sync.equip("player", b"skin", Variant::Classic, now);
        sync.saved("player", Some(&look(b"skin", Variant::Classic)));
        sync.cancel();
        assert!(sync.take_ready(now + DEBOUNCE).is_none());
    }
}
