//! Background jobs for the Skins tab: the Arctic look (published to other
//! Arctic players), the account's Minecraft skin and capes (Mojang, needs
//! Microsoft), looking up other players and picking files.

use std::collections::HashSet;
use std::path::Path;

#[cfg(feature = "offline-accounts")]
use arctic_core::Error;
use arctic_core::Result as CoreResult;
use arctic_core::auth::{Account, AccountKind};
use arctic_core::cosmetic_models;
use arctic_core::cosmetics::{self, CapeChoice, Look, NewLook, Preset, Texture};
use arctic_core::skins::Variant;
use arctic_core::skins::api::{self, Profile};

use crate::tasks::{Event, Tasks};
use crate::{t, tf};
/// The player's Arctic look and the preset capes, textures included.
#[derive(Debug, Clone)]
pub struct ArcticState {
    pub presets: Vec<Preset>,
    pub look: Look,
    /// (texture hash, PNG) for the presets and the current look.
    pub textures: Vec<(String, Vec<u8>)>,
}

#[derive(Debug, Default)]
pub struct CosmeticCatalog {
    /// 3D cosmetics with their parsed models (items that failed are left out).
    pub items: Vec<(cosmetic_models::Item, cosmetic_models::Geometry)>,
    /// Sculpted cosmetics (mesh-only ones, and the meshes of cuboid items that also have one).
    pub meshes: Vec<(
        cosmetic_models::MeshItem,
        std::sync::Arc<cosmetic_models::Mesh>,
    )>,
    pub textures: Vec<(String, Vec<u8>)>,
}

#[derive(Debug)]
pub struct LoadedArcticState {
    pub look: ArcticState,
    pub catalog: Result<CosmeticCatalog, String>,
}

#[derive(Default)]
pub struct CatalogState {
    pub catalog: Option<CosmeticCatalog>,
    pub error: Option<String>,
}

pub fn merge_catalog(
    previous: Option<CosmeticCatalog>,
    result: Result<CosmeticCatalog, String>,
) -> CatalogState {
    match result {
        Ok(catalog) => CatalogState {
            catalog: Some(catalog),
            error: None,
        },
        Err(error) => CatalogState {
            catalog: previous,
            error: Some(error),
        },
    }
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

#[derive(Debug)]
pub struct SyncedMinecraftSkin {
    pub profile: Profile,
    pub png: Vec<u8>,
    pub undo: Option<SkinChange>,
}

/// What to change on Mojang's side before re-reading the profile.
#[derive(Debug, Clone)]
pub enum SkinChange {
    None,
    Upload(Variant, Vec<u8>),
    Reset,
    Cape(Option<String>),
}

fn previous_skin_change(
    profile: &Profile,
    texture: impl FnOnce(&str) -> CoreResult<Vec<u8>>,
) -> CoreResult<SkinChange> {
    match profile.active_skin() {
        Some(skin) => Ok(SkinChange::Upload(skin.variant(), texture(&skin.url)?)),
        None => Ok(SkinChange::Reset),
    }
}

impl Tasks {
    /// Load the public cape and cosmetic catalog for browsing without an account.
    pub fn arctic_catalog(&self) {
        self.run(|t| {
            let base = cosmetics::base_url();
            let result = t
                .load_catalog_blocking(&base, Look::default())
                .map_err(|e| e.to_string());
            t.send(Event::ArcticCatalog(result));
        });
    }

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

    pub fn sync_minecraft_skin(
        &self,
        account: Account,
        variant: Variant,
        png: Vec<u8>,
        capture_previous: bool,
    ) {
        self.run(move |t| {
            let id = account.id.clone();
            let result = (|| -> CoreResult<_> {
                let token = t.fresh_token(account)?;
                let undo = if capture_previous {
                    let profile = api::profile(&token)?;
                    Some(previous_skin_change(&profile, api::texture)?)
                } else {
                    None
                };
                let profile = api::upload_skin(&token, variant, &png)?;
                Ok(SyncedMinecraftSkin { profile, png, undo })
            })()
            .map_err(|e| e.to_string());
            t.send(Event::MinecraftSkinSynced(
                t.dirs().profile_root().to_owned(),
                id,
                result,
            ));
        });
    }

    fn skin_account_blocking(
        &self,
        account: Account,
        change: SkinChange,
    ) -> CoreResult<AccountSkin> {
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
    fn fresh_token(&self, account: Account) -> CoreResult<String> {
        match self.fresh_account(account)?.kind {
            AccountKind::Microsoft(session) => Ok(session.access_token.reveal().to_string()),
            #[cfg(feature = "offline-accounts")]
            AccountKind::Offline => Err(Error::Other(
                t!("tasks.minecraft_skins_can_only_be_changed_on").into(),
            )),
        }
    }

    pub fn arctic_look(&self, account: Account) {
        self.run(move |t| {
            let id = account.id.clone();
            let result = t.arctic_look_blocking(account).map_err(|e| e.to_string());
            t.send(Event::ArcticLook(id, result));
        });
    }

    /// Save the look and fetch only its missing textures, so trying things on stays quick.
    pub fn save_arctic_look(&self, account: Account, change: NewLook, known: ArcticState) {
        self.run(move |t| {
            let id = account.id.clone();
            let result = t
                .save_look_blocking(account, change, known)
                .map_err(|e| e.to_string());
            t.send(Event::ArcticLookSaved(id, result));
        });
    }

    fn save_look_blocking(
        &self,
        account: Account,
        change: NewLook,
        known: ArcticState,
    ) -> CoreResult<ArcticState> {
        let base = cosmetics::base_url();
        let account = self.fresh_account(account)?;
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
        let look = cosmetics::with_session(self.dirs(), &base, &account, |token| {
            cosmetics::set_look(&base, token, &change)
        })?;
        for hash in look.skin.iter().chain(look.cape.iter()) {
            if !textures.iter().any(|(h, _)| h == hash) {
                textures.push((hash.clone(), cosmetics::texture(&base, hash)?));
            }
        }
        Ok(ArcticState {
            presets: known.presets,
            look,
            textures,
        })
    }

    fn arctic_look_blocking(&self, account: Account) -> CoreResult<LoadedArcticState> {
        let base = cosmetics::base_url();
        let account = self.fresh_account(account)?;
        let look = cosmetics::with_session(self.dirs(), &base, &account, |token| {
            cosmetics::my_look(&base, token)
        })?;
        self.load_catalog_blocking(&base, look)
    }

    fn load_catalog_blocking(&self, base: &str, look: Look) -> CoreResult<LoadedArcticState> {
        let presets = cosmetics::catalog(base)?;
        // Models and textures never change once uploaded (they're named by
        // their hash): kept on disk, and fetched several at a time.
        let cache = self.dirs().cache().join("arctic-looks");
        let catalog = load_cosmetic_catalog(base, &cache).map_err(|e| {
            log::warn!("3D cosmetics catalog: {e}");
            e.to_string()
        });
        let mut wanted: Vec<String> = presets.iter().map(|p| p.texture.clone()).collect();
        // Each animated cape's still image too (worn when its wearer turns Animate off).
        wanted.extend(presets.iter().filter_map(|p| p.still.clone()));
        wanted.extend(look.skin.clone());
        wanted.extend(look.cape.clone());
        let textures = load_textures(base, &cache, wanted);
        Ok(LoadedArcticState {
            look: ArcticState {
                presets,
                look,
                textures,
            },
            catalog,
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
            let result = pick_png(t!("tasks.choose_a_skin"), t!("tasks.minecraft_skin"));
            t.send(Event::SkinFile(result));
        });
    }

    /// Save the same bytes we received, including legacy 64×32 PNGs.
    pub fn save_skin_png(&self, name: String, png: Option<Vec<u8>>, gallery_hash: Option<String>) {
        self.run(move |t| {
            let result = save_skin_png(&name, png, gallery_hash.as_deref(), |filename| {
                rfd::FileDialog::new()
                    .set_title(t!("skins.download_as_png"))
                    .add_filter("PNG", &["png"])
                    .set_file_name(filename)
                    .save_file()
            });
            t.send(Event::SkinPngSaved(result));
        });
    }

    /// Look up the OptiFine cape of a player name.
    pub fn optifine_cape(&self, name: String) {
        self.run(move |t| {
            let result = arctic_core::skins::optifine::fetch(&name).map_err(|e| e.to_string());
            t.send(Event::OptifineCape(name, result));
        });
    }

    /// Ask for a cape image; `None` when the dialog was cancelled.
    pub fn pick_cape_file(&self) {
        self.run(|t| {
            let result = pick_cape_image(
                t!("tasks.choose_a_cape_image_64_32_up_to_1024_512_or"),
                t!("tasks.cape"),
            );
            t.send(match result {
                Ok(Some((bytes, true))) => Event::CapeGif(Ok(Some(bytes))),
                Ok(Some((bytes, false))) => Event::CapeFile(Ok(Some(bytes))),
                Ok(None) => Event::CapeFile(Ok(None)),
                Err(e) => Event::CapeFile(Err(e)),
            });
        });
    }
}

fn load_cosmetic_catalog(base: &str, cache: &Path) -> CoreResult<CosmeticCatalog> {
    let catalog = cosmetic_models::catalog(base)?;
    // Meshes: the mesh-only items, and the sculpted version of any cuboid item that has one.
    let mut mesh_items = catalog.meshes;
    mesh_items.extend(catalog.cosmetics.iter().filter_map(|i| {
        i.mesh.clone().map(|mesh| cosmetic_models::MeshItem {
            id: i.id.clone(),
            name: i.name.clone(),
            slot: i.slot.clone(),
            mesh,
        })
    }));
    let meshes = in_parallel(mesh_items, |item| {
        let bytes = cached(cache, &item.mesh, || {
            cosmetic_models::mesh_asset(base, &item.mesh)
        })?;
        let mesh = cosmetic_models::mesh::parse(&bytes)
            .inspect_err(|e| log::warn!("cosmetic {} (mesh): {e}", item.id))
            .ok()?;
        Some((item, std::sync::Arc::new(mesh)))
    });
    let items = in_parallel(catalog.cosmetics, |item| {
        let bytes = cached(cache, &item.model, || {
            cosmetic_models::asset(base, &item.model)
        })?;
        let geometry = cosmetic_models::Geometry::parse(&bytes)
            .inspect_err(|e| log::warn!("cosmetic {}: {e}", item.id))
            .ok()?;
        Some((item, geometry))
    });
    let textures = load_textures(
        base,
        cache,
        items.iter().map(|(i, _)| i.texture.clone()).collect(),
    );
    Ok(CosmeticCatalog {
        items,
        meshes,
        textures,
    })
}

fn load_textures(base: &str, cache: &Path, mut wanted: Vec<String>) -> Vec<(String, Vec<u8>)> {
    let mut seen = HashSet::new();
    wanted.retain(|h| seen.insert(h.clone()));
    in_parallel(wanted, |h| {
        let png = cached(cache, &h, || cosmetics::texture(base, &h))?;
        Some((h, png))
    })
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
            .map_err(|e| tf!("tasks.could_not_read", path = path.display(), e = e)),
    }
}

/// A cape image: a PNG as it is, or a GIF converted to the cape strip exactly as the server
/// would (so the preview shows what players get, and the server's errors show here too).
/// The flag says the file was a GIF.
fn pick_cape_image(title: &str, filter: &str) -> Result<Option<(Vec<u8>, bool)>, String> {
    let picked = rfd::FileDialog::new()
        .set_title(title)
        .add_filter(filter, &["png", "gif"])
        .pick_file();
    let Some(path) = picked else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path)
        .map_err(|e| tf!("tasks.could_not_read", path = path.display(), e = e))?;
    if arctic_core::skins::gif_cape::is_gif(&bytes) {
        arctic_core::skins::gif_cape::to_cape_png(&bytes).map(|png| Some((png, true)))
    } else {
        Ok(Some((bytes, false)))
    }
}

fn save_skin_png(
    name: &str,
    png: Option<Vec<u8>>,
    gallery_hash: Option<&str>,
    choose_path: impl FnOnce(String) -> Option<std::path::PathBuf>,
) -> Result<Option<()>, String> {
    let png = match (png, gallery_hash) {
        (Some(png), _) => png,
        (None, Some(hash)) => {
            cosmetics::texture(&cosmetics::base_url(), hash).map_err(|e| e.to_string())?
        }
        (None, None) => return Err(t!("skins.could_not_get_skin").to_owned()),
    };
    arctic_core::skins::decode(&png).map_err(|e| e.to_string())?;
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim_matches([' ', '.']);
    let name = if name.is_empty() {
        t!("tasks.skin")
    } else {
        name
    };
    let Some(path) = choose_path(format!("{name}.png")) else {
        return Ok(None);
    };
    std::fs::write(path, png).map_err(|e| e.to_string())?;
    Ok(Some(()))
}

pub fn file_stem(path: &std::path::Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| t!("tasks.skin").into())
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
    get: impl FnOnce() -> CoreResult<Vec<u8>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use cosmetic_models::{Geometry, Item, Mesh, MeshItem};
    use std::sync::Arc;

    #[test]
    fn undo_uses_only_the_active_skin_and_its_original_variant() {
        let mut profile = Profile {
            id: "player".into(),
            name: "Alex".into(),
            skins: vec![],
            capes: vec![],
        };
        assert!(matches!(
            previous_skin_change(&profile, |_| panic!("default has no texture")).unwrap(),
            SkinChange::Reset
        ));
        profile.skins.push(api::ProfileSkin {
            id: "old".into(),
            state: "ACTIVE".into(),
            url: "https://textures.minecraft.net/texture/old".into(),
            variant: "SLIM".into(),
        });
        let undo = previous_skin_change(&profile, |url| {
            assert_eq!(url, profile.skins[0].url);
            Ok(b"previous".to_vec())
        })
        .unwrap();
        assert!(matches!(undo, SkinChange::Upload(Variant::Slim, png) if png == b"previous"));
    }

    #[test]
    fn failed_catalog_load_keeps_assets_until_a_successful_refresh() {
        let catalog = CosmeticCatalog {
            items: vec![(
                Item {
                    id: "hat".into(),
                    name: "hat".into(),
                    slot: "head".into(),
                    model: "model".into(),
                    texture: "texture".into(),
                    animation: None,
                    mesh: None,
                },
                Geometry {
                    texture_width: 16,
                    texture_height: 16,
                    bones: vec![],
                },
            )],
            meshes: vec![(
                MeshItem {
                    id: "pet".into(),
                    name: "pet".into(),
                    slot: "shoulders".into(),
                    mesh: "mesh".into(),
                },
                Arc::new(Mesh {
                    nodes: vec![],
                    primitives: vec![],
                    materials: vec![],
                    images: vec![vec![1, 2, 3]],
                    animation: None,
                    sheen: None,
                }),
            )],
            textures: vec![("texture".into(), vec![4, 5, 6])],
        };

        let failed = merge_catalog(None, Err("timeout".into()));
        assert!(failed.catalog.is_none());
        assert_eq!(failed.error.as_deref(), Some("timeout"));

        let good = merge_catalog(failed.catalog, Ok(catalog));
        let failed = merge_catalog(good.catalog, Err("timeout".into()));
        assert_eq!(failed.error.as_deref(), Some("timeout"));
        let retained = failed.catalog.unwrap();
        assert_eq!(retained.items[0].0.id, "hat");
        assert_eq!(retained.meshes[0].1.images, [vec![1, 2, 3]]);
        assert_eq!(retained.textures, [("texture".into(), vec![4, 5, 6])]);

        let empty = merge_catalog(Some(retained), Ok(CosmeticCatalog::default()));
        assert!(empty.error.is_none());
        let empty = empty.catalog.unwrap();
        assert!(empty.items.is_empty());
        assert!(empty.meshes.is_empty());
        assert!(empty.textures.is_empty());
    }
}

#[cfg(test)]
mod save_tests {
    use image::ImageEncoder;

    use super::save_skin_png;

    #[test]
    fn save_preserves_modern_and_legacy_png_bytes() {
        let dir = tempfile::tempdir().unwrap();
        for height in [32, 64] {
            let mut original = Vec::new();
            image::codecs::png::PngEncoder::new_with_quality(
                &mut original,
                image::codecs::png::CompressionType::Fast,
                image::codecs::png::FilterType::NoFilter,
            )
            .write_image(
                &vec![0; 64 * height as usize * 4],
                64,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();

            let path = dir.path().join(format!("skin-{height}.png"));
            let result = save_skin_png("Test Skin", Some(original.clone()), None, |filename| {
                assert_eq!(filename, "Test Skin.png");
                Some(path.clone())
            });
            assert_eq!(result, Ok(Some(())));
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
    }
}
