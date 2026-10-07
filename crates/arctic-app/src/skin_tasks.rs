//! Background jobs for the Skins tab: the Arctic look (published to other
//! Arctic players), the account's Minecraft skin and capes (Mojang, needs
//! Microsoft), looking up other players and picking files.

use std::collections::HashSet;

#[cfg(feature = "offline-accounts")]
use arctic_core::Error;
use arctic_core::Result;
use arctic_core::auth::microsoft::{self, MsaConfig};
use arctic_core::auth::{Account, AccountKind, now_secs};
use arctic_core::cosmetic_models;
use arctic_core::cosmetics::{self, CapeChoice, Look, NewLook, Preset, Texture};
use arctic_core::skins::Variant;
use arctic_core::skins::api::{self, Profile};

use crate::tasks::{Event, Tasks};

/// The player's Arctic look and the preset capes, textures included.
#[derive(Debug, Clone)]
pub struct ArcticState {
    pub presets: Vec<Preset>,
    pub look: Look,
    /// (texture hash, PNG) for the presets, cosmetics and the current look.
    pub textures: Vec<(String, Vec<u8>)>,
    /// 3D cosmetics with their parsed models (items that failed are left out).
    pub items: Vec<(cosmetic_models::Item, cosmetic_models::Geometry)>,
    /// Sculpted cosmetics (mesh-only ones, and the meshes of cuboid items that also have one).
    pub meshes: Vec<(cosmetic_models::MeshItem, std::sync::Arc<cosmetic_models::Mesh>)>,
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
    pub(crate) fn fresh_account(&self, account: Account) -> Result<Account> {
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
            AccountKind::Microsoft(session) => Ok(session.access_token.reveal().to_string()),
            #[cfg(feature = "offline-accounts")]
            AccountKind::Offline => Err(Error::Other(
                "Minecraft skins can only be changed on Microsoft accounts.".into(),
            )),
        }
    }

    /// Read (after optionally publishing a new one) the Arctic look.
    /// Load the Arctic look, or publish `change`. With `known` (the state
    /// shown now) a change only saves the look and fetches textures that
    /// aren't there yet, so trying things on stays quick.
    pub fn arctic_look(
        &self,
        account: Account,
        change: Option<NewLook>,
        known: Option<ArcticState>,
    ) {
        self.run(move |t| {
            let id = account.id.clone();
            let result = match (change, known) {
                (Some(change), Some(known)) => t.save_look_blocking(account, change, known),
                (change, _) => t.arctic_look_blocking(account, change),
            }
            .map_err(|e| e.to_string());
            t.send(Event::ArcticLook(id, result));
        });
    }

    fn save_look_blocking(
        &self,
        account: Account,
        change: NewLook,
        known: ArcticState,
    ) -> Result<ArcticState> {
        let base = cosmetics::base_url();
        let account = self.fresh_account(account)?;
        let token = cosmetics::token_for(self.dirs(), &base, &account)?;
        // What's uploaded now is already here: no need to download it back.
        let mut textures = known.textures;
        let uploaded = [
            change.skin.as_ref().map(|(t, _)| t),
            change.cape.as_ref().and_then(|c| match c {
                CapeChoice::Custom(t) => Some(t),
                CapeChoice::Preset(_) => None,
            }),
        ];
        for texture in uploaded.into_iter().flatten() {
            if let Texture::Png(png) = texture {
                textures.push((cosmetics::texture_hash(png), png.clone()));
            }
        }
        let look = cosmetics::set_look(&base, &token, &change)?;
        for hash in look.skin.iter().chain(look.cape.iter()) {
            if !textures.iter().any(|(h, _)| h == hash) {
                textures.push((hash.clone(), cosmetics::texture(&base, hash)?));
            }
        }
        Ok(ArcticState {
            presets: known.presets,
            look,
            textures,
            items: known.items,
            meshes: known.meshes,
        })
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
        // Models and textures never change once uploaded (they're named by
        // their hash): kept on disk, and fetched several at a time.
        let cache = self.dirs().cache().join("arctic-looks");
        // 3D cosmetics are optional: an older server just has none.
        let catalog = cosmetic_models::catalog(&base).unwrap_or_default();
        // Meshes: the mesh-only items, and the sculpted version of any cuboid item that has one.
        let mut mesh_items = catalog.meshes.clone();
        mesh_items.extend(catalog.cosmetics.iter().filter_map(|i| {
            i.mesh.clone().map(|mesh| cosmetic_models::MeshItem {
                id: i.id.clone(),
                name: i.name.clone(),
                slot: i.slot.clone(),
                mesh,
            })
        }));
        let meshes = in_parallel(mesh_items, |item| {
            let bytes = cached(&cache, &item.mesh, || {
                cosmetic_models::mesh_asset(&base, &item.mesh)
            })?;
            let mesh = cosmetic_models::mesh::parse(&bytes)
                .inspect_err(|e| log::warn!("cosmetic {} (mesh): {e}", item.id))
                .ok()?;
            Some((item, std::sync::Arc::new(mesh)))
        });
        let items: Vec<_> = in_parallel(catalog.cosmetics, |item| {
            let bytes = cached(&cache, &item.model, || {
                cosmetic_models::asset(&base, &item.model)
            })?;
            let geometry = cosmetic_models::Geometry::parse(&bytes)
                .inspect_err(|e| log::warn!("cosmetic {}: {e}", item.id))
                .ok()?;
            Some((item, geometry))
        });
        let mut wanted: Vec<String> = presets.iter().map(|p| p.texture.clone()).collect();
        // Each animated cape's still image too (worn when its wearer turns Animate off).
        wanted.extend(presets.iter().filter_map(|p| p.still.clone()));
        wanted.extend(items.iter().map(|(i, _)| i.texture.clone()));
        wanted.extend(look.skin.clone());
        wanted.extend(look.cape.clone());
        let mut seen = HashSet::new();
        wanted.retain(|h| seen.insert(h.clone()));
        let textures = in_parallel(wanted, |h| {
            let png = cached(&cache, &h, || cosmetics::texture(&base, &h))?;
            Some((h, png))
        });
        Ok(ArcticState {
            presets,
            look,
            textures,
            items,
            meshes,
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
                "Choose a cape image (64×32 up to 1024×512, or up to 32 frames stacked for an animated cape)",
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

/// How many looks-server downloads run at once.
const PARALLEL_DOWNLOADS: usize = 8;

/// `f` over `items` on a few threads; results in the items' order, failures left out.
pub(crate) fn in_parallel<T: Send, R: Send>(
    items: Vec<T>,
    f: impl Fn(T) -> Option<R> + Sync,
) -> Vec<R> {
    let jobs = std::sync::Mutex::new(items.into_iter().enumerate());
    let done = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..PARALLEL_DOWNLOADS {
            scope.spawn(|| {
                loop {
                    let next = jobs.lock().ok().and_then(|mut j| j.next());
                    let Some((i, item)) = next else { break };
                    if let Some(result) = f(item)
                        && let Ok(mut done) = done.lock()
                    {
                        done.push((i, result));
                    }
                }
            });
        }
    });
    let mut done = done.into_inner().unwrap_or_default();
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

/// A file named by its hash: from the disk cache, else downloaded (and kept
/// when its SHA-1 is that hash, as textures' are).
pub(crate) fn cached(
    dir: &std::path::Path,
    hash: &str,
    get: impl FnOnce() -> Result<Vec<u8>>,
) -> Option<Vec<u8>> {
    let valid = hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit());
    let path = dir.join(hash);
    if valid && let Ok(bytes) = std::fs::read(&path) {
        return Some(bytes);
    }
    let bytes = get().ok()?;
    if valid && cosmetics::texture_hash(&bytes) == hash.to_ascii_lowercase() {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(&path, &bytes);
    }
    Some(bytes)
}
