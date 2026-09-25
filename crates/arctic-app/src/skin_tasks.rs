//! Background jobs for the Skins tab: reading and changing the account's
//! skin and cape, looking up other players and picking files.

use arctic_core::auth::microsoft::{self, MsaConfig};
use arctic_core::auth::{Account, AccountKind, now_secs};
use arctic_core::skins::Variant;
use arctic_core::skins::api::{self, Profile};
use arctic_core::{Error, Result};

use crate::tasks::{Event, Tasks};

/// The signed-in player's skin state with textures downloaded.
#[derive(Debug, Clone)]
pub struct AccountSkin {
    pub profile: Profile,
    /// Active skin PNG (`None` = default skin).
    pub skin_png: Option<Vec<u8>>,
    /// (cape id, PNG) for every cape the player owns.
    pub capes: Vec<(String, Vec<u8>)>,
}

/// What to change before re-reading the profile.
#[derive(Debug, Clone)]
pub enum SkinChange {
    None,
    Upload(Variant, Vec<u8>),
    Reset,
    Cape(Option<String>),
}

impl Tasks {
    /// Read (after optionally changing) the account's skin and capes.
    pub fn skin_account(&self, account: Account, change: SkinChange) {
        self.run(move |t| {
            let id = account.id.clone();
            let result = t
                .skin_account_blocking(account, change)
                .map_err(|e| e.to_string());
            t.send(Event::SkinAccount(id, result));
        });
    }

    fn skin_account_blocking(&self, account: Account, change: SkinChange) -> Result<AccountSkin> {
        let token = self.fresh_token(account)?;
        let profile = match change {
            SkinChange::None => api::profile(&token)?,
            SkinChange::Upload(variant, png) => api::upload_skin(&token, variant, &png)?,
            SkinChange::Reset => api::reset_skin(&token)?,
            SkinChange::Cape(id) => api::set_cape(&token, id.as_deref())?,
        };
        let skin_png = match profile.active_skin() {
            Some(skin) => Some(api::texture(&skin.url)?),
            None => None,
        };
        let capes = profile
            .capes
            .iter()
            .filter_map(|c| api::texture(&c.url).ok().map(|png| (c.id.clone(), png)))
            .collect();
        Ok(AccountSkin {
            profile,
            skin_png,
            capes,
        })
    }

    /// A valid Minecraft access token, refreshing the login if needed.
    fn fresh_token(&self, account: Account) -> Result<String> {
        let account = if account.needs_refresh(now_secs()) {
            let cfg = MsaConfig::load(self.dirs())?;
            let fresh = microsoft::refresh(&cfg, &account)?;
            self.send(Event::AccountRefreshed(fresh.clone()));
            fresh
        } else {
            account
        };
        match account.kind {
            AccountKind::Microsoft(session) => Ok(session.access_token),
            AccountKind::Offline => Err(Error::Other(
                "Skins can only be changed on Microsoft accounts.".into(),
            )),
        }
    }

    /// Another player's current skin, by name.
    pub fn player_skin(&self, name: String) {
        self.run(move |t| {
            let result = api::player_skin(&name).map_err(|e| e.to_string());
            t.send(Event::PlayerSkin(name, result));
        });
    }

    /// Ask for a skin file; `None` when the dialog was cancelled.
    pub fn pick_skin_file(&self) {
        self.run(|t| {
            let picked = rfd::FileDialog::new()
                .set_title("Choose a skin")
                .add_filter("Minecraft skin", &["png"])
                .pick_file();
            let result = match picked {
                None => Ok(None),
                Some(path) => std::fs::read(&path)
                    .map(|bytes| Some((file_stem(&path), bytes)))
                    .map_err(|e| format!("Could not read {}: {e}", path.display())),
            };
            t.send(Event::SkinFile(result));
        });
    }
}

pub fn file_stem(path: &std::path::Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Skin".into())
}
