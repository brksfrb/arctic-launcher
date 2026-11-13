//! Background jobs for sharing: making a code, saving and opening bundle
//! files, fetching a code, and importing a bundle.

use arctic_core::auth::Account;
use arctic_core::cosmetics;
use arctic_core::instances::Instance;
use arctic_core::sharing::{self, Bundle};
use arctic_core::{Error, ProgressInfo, Result};

use crate::tasks::{Event, ProgressSnapshot, Tasks};
use crate::{t, tf};
/// What an import did, for the UI.
#[derive(Debug, Clone)]
pub struct Imported {
    pub title: String,
    pub detail: String,
    /// Mods that couldn't be installed, instances left alone…
    pub warnings: Vec<String>,
    /// Instance to show afterwards.
    pub open: Option<String>,
    /// Launcher settings changed (profile imports).
    pub settings_changed: bool,
}

impl Tasks {
    /// Upload `bundle` and get a short code for it.
    pub fn share_code(&self, bundle: Bundle, account: Account) {
        self.run(move |t| {
            let result = t.make_code(&bundle, account).map_err(|e| e.to_string());
            t.send(Event::ShareCode(result));
        });
    }

    fn make_code(&self, bundle: &Bundle, account: Account) -> Result<String> {
        let account = self.fresh_account(account)?;
        let base = cosmetics::base_url();
        let token = cosmetics::token_for(self.dirs(), &base, &account)?;
        sharing::codes::create(&base, &token, bundle)
    }

    /// Ask where to save `bundle` as a .json file.
    pub fn share_save(&self, bundle: Bundle, file_name: String) {
        self.run(move |t| {
            let picked = rfd::FileDialog::new()
                .set_title(t!("tasks.save_share_file"))
                .set_file_name(&file_name)
                .add_filter(t!("tasks.arctic_share"), &["json"])
                .save_file();
            let result = match picked {
                None => Ok(None),
                Some(path) => std::fs::write(&path, bundle.to_file_text())
                    .map(|()| Some(path.clone()))
                    .map_err(|e| tf!("tasks.could_not_save", path = path.display(), e = e)),
            };
            t.send(Event::ShareSaved(result));
        });
    }

    /// Ask for a share file and read it.
    pub fn share_open_file(&self) {
        self.run(|t| {
            let picked = rfd::FileDialog::new()
                .set_title(t!("tasks.open_share_file"))
                .add_filter(t!("tasks.arctic_share"), &["json", "txt"])
                .pick_file();
            t.note_if_picker_missing(&picked);
            let result = match picked {
                None => Ok(None),
                Some(path) => sharing::read_file(&path)
                    .map(Some)
                    .map_err(|e| e.to_string()),
            };
            t.send(Event::ShareLoaded(result));
        });
    }

    /// Fetch the bundle behind a code.
    pub fn share_fetch(&self, code: String) {
        self.run(move |t| {
            let result = sharing::codes::fetch(&cosmetics::base_url(), &code)
                .map(Some)
                .map_err(|e| e.to_string());
            t.send(Event::ShareLoaded(result));
        });
    }

    /// Mod files put in the mods folder by hand (from another launcher or folder): looked up on
    /// Modrinth by their hash, so the ones it has are shared like mods installed here.
    pub fn share_look_up_mods(&self, instance: Instance, game_version: String) {
        self.run(move |t| {
            let result = match instance.loader.kind() {
                Some(loader) => arctic_core::mods::recognize(
                    &instance.game_dir(t.dirs()).join("mods"),
                    &arctic_core::mods::index_path(&t.dirs().instance_dir(&instance.id)),
                    &game_version,
                    loader,
                )
                .map(|r| r.tracked)
                .map_err(|e| e.to_string()),
                None => Ok(0),
            };
            t.send(Event::ShareModsLookedUp(instance.id, result));
        });
    }

    /// Use a bundle: new instance, client settings into `target`, or a
    /// whole profile.
    pub fn share_import(&self, bundle: Bundle, target: Option<Instance>) {
        self.run(move |t| {
            let progress =
                |p: ProgressInfo| t.send(Event::ShareProgress(ProgressSnapshot::from(p)));
            let result = import(t.dirs(), bundle, target, &progress).map_err(|e| e.to_string());
            t.send(Event::ShareImported(result));
        });
    }
}

fn import(
    dirs: &arctic_core::storage::DataDirs,
    bundle: Bundle,
    target: Option<Instance>,
    progress: arctic_core::Progress,
) -> Result<Imported> {
    match bundle {
        Bundle::Instance(pack) => {
            let (instance, report) = pack.install(dirs, progress)?;
            Ok(Imported {
                title: tf!("tasks.added_2", name = instance.name),
                detail: tf!(
                    "tasks.mods_installed_press_play_when_youre_ready",
                    count = report.installed.len()
                ),
                warnings: skipped(&report.skipped),
                open: Some(instance.id),
                settings_changed: false,
            })
        }
        Bundle::Client(part) => {
            let target =
                target.ok_or_else(|| Error::Other("pick an instance to apply it to".into()))?;
            part.apply(&target.game_dir(dirs))?;
            Ok(Imported {
                title: tf!("tasks.applied_to", name = target.name),
                detail: t!("tasks.youll_see_it_next_time_you_play").into(),
                warnings: Vec::new(),
                open: None,
                settings_changed: false,
            })
        }
        Bundle::Options(defaults) => {
            defaults.save(dirs)?;
            Ok(Imported {
                title: t!("tasks.default_game_settings_saved").into(),
                detail: tf!(
                    "tasks.settings_go_into_every_new_instances_first",
                    count = defaults.values.len()
                ),
                warnings: Vec::new(),
                open: None,
                settings_changed: true,
            })
        }
        Bundle::Profile(pack) => {
            let report = pack.import(dirs, progress)?;
            let mut warnings = skipped(&report.skipped_mods);
            warnings.extend(
                report
                    .existing
                    .iter()
                    .map(|n| tf!("tasks.kept_your_own_same_name", n_value = n)),
            );
            Ok(Imported {
                title: t!("tasks.profile_imported").into(),
                detail: tf!(
                    "tasks.settings_applied_instance_s_added",
                    count = report.created.len()
                ),
                warnings,
                open: None,
                settings_changed: true,
            })
        }
    }
}

fn skipped(list: &[arctic_core::mods::Skipped]) -> Vec<String> {
    list.iter()
        .map(|s| tf!("tasks.skipped", title = s.title, reason = s.reason))
        .collect()
}
