//! Background jobs for the Skins tab: the Arctic look (published to other
//! Arctic players), the account's Minecraft skin and capes (Mojang, needs
//! Microsoft), looking up other players and picking files.

use std::collections::HashSet;

use arctic_core::auth::microsoft::{self, MsaConfig};
use arctic_core::auth::{Account, AccountKind, now_secs};
use arctic_core::cosmetics::{self, Look, NewLook, Preset};
use arctic_core::skins::Variant;
use arctic_core::skins::api::{self, Profile};
use arctic_core::{Error, Result};

use crate::tasks::{Event, Tasks};

/// The player's Arctic look and the preset capes, textures included.
#[derive(Debug, Clone)]
pub struct ArcticState {
    pub presets: Vec<Preset>,
    pub look: Look,
    /// (texture hash, PNG) for the presets and the current look.
    pub textures: Vec<(String, Vec<u8>)>,
}

impl ArcticState {
    pub fn png(&self, hash: &str) -> Option<&[u8]> {
        self.textures
            .iter()
            .find(|(h, _)| h == hash)
            .map(|(_, png)| png.as_slice())
    }

    /// The current look as something that can be published again, so one
    /// part can change while the rest stays.
    pub fn current(&self) -> NewLook {
        NewLook::from_look(&self.look, &self.presets)
    }
}

/// The account's Minecraft (Mojang) skin state with textures downloaded.
#[derive(Debug, Clone)]
pub struct AccountSkin {
    pub profile: Profile,
    /// Active skin PNG (`None` = default skin).
    pub skin_png: Option<Vec<u8>>,
    /// (cape id, PNG) for every cape the player owns.
    pub capes: Vec<(String, Vec<u8>)>,
}

/// What to change on Mojang's side before re-reading the profile.
#[derive(Debug, Clone)]
pub enum SkinChange {
    None,
    Upload(Variant, Vec<u8>),
    Reset,
    Cape(Option<String>),
}

impl Tasks {
    /// Read (after optionally changing) the account's Minecraft skin and capes.
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

    /// The account refreshed if needed (Microsoft tokens expire).
    fn fresh_account(&self, account: Account) -> Result<Account> {
        if account.needs_refresh(now_secs()) {
            let cfg = MsaConfig::load(self.dirs())?;
            let fresh = microsoft::refresh(&cfg, &account)?;
            self.send(Event::AccountRefreshed(fresh.clone()));
            Ok(fresh)
        } else {
            Ok(account)
        }
    }

    /// A valid Minecraft access token, refreshing the login if needed.
    fn fresh_token(&self, account: Account) -> Result<String> {
        match self.fresh_account(account)?.kind {
            AccountKind::Microsoft(session) => Ok(session.access_token),
            AccountKind::Offline => Err(Error::Other(
                "Minecraft skins can only be changed on Microsoft accounts.".into(),
            )),
        }
    }

    /// Read (after optionally publishing a new one) the Arctic look.
    pub fn arctic_look(&self, account: Account, change: Option<NewLook>) {
        self.run(move |t| {
            let id = account.id.clone();
            let result = t
                .arctic_look_blocking(account, change)
                .map_err(|e| e.to_string());
            t.send(Event::ArcticLook(id, result));
        });
    }

    fn arctic_look_blocking(
        &self,
        account: Account,
        change: Option<NewLook>,
    ) -> Result<ArcticState> {
        let base = cosmetics::base_url();
        let presets = cosmetics::catalog(&base)?;
        let account = self.fresh_account(account)?;
        let token = cosmetics::token_for(self.dirs(), &base, &account)?;
        let look = match change {
            Some(new) => cosmetics::set_look(&base, &token, &new)?,
            None => cosmetics::my_look(&base, &token)?,
        };
        let mut wanted: Vec<String> = presets.iter().map(|p| p.texture.clone()).collect();
        wanted.extend(look.skin.clone());
        wanted.extend(look.cape.clone());
        let mut seen = HashSet::new();
        let textures = wanted
            .into_iter()
            .filter(|h| seen.insert(h.clone()))
            .filter_map(|h| cosmetics::texture(&base, &h).ok().map(|png| (h, png)))
            .collect();
        Ok(ArcticState {
            presets,
            look,
            textures,
        })
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
            let result = pick_png("Choose a skin", "Minecraft skin");
            t.send(Event::SkinFile(result));
        });
    }

    /// Ask for a cape image; `None` when the dialog was cancelled.
    pub fn pick_cape_file(&self) {
        self.run(|t| {
            let result = pick_png(
                "Choose a cape image (64×32, or up to 8 frames stacked for an animated cape)",
                "Cape",
            )
            .map(|f| f.map(|(_, b)| b));
            t.send(Event::CapeFile(result));
        });
    }
}

type Picked = std::result::Result<Option<(String, Vec<u8>)>, String>;

fn pick_png(title: &str, filter: &str) -> Picked {
    let picked = rfd::FileDialog::new()
        .set_title(title)
        .add_filter(filter, &["png"])
        .pick_file();
    match picked {
        None => Ok(None),
        Some(path) => std::fs::read(&path)
            .map(|bytes| Some((file_stem(&path), bytes)))
            .map_err(|e| format!("Could not read {}: {e}", path.display())),
    }
}

pub fn file_stem(path: &std::path::Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Skin".into())
}
