//! Background jobs for sharing: making a code, saving and opening bundle
//! files, fetching a code, and importing a bundle.

use arctic_core::auth::Account;
use arctic_core::cosmetics;
use arctic_core::instances::Instance;
use arctic_core::sharing::{self, Bundle};
use arctic_core::{Error, ProgressInfo, Result};

use crate::tasks::{Event, ProgressSnapshot, Tasks};

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
                .set_title("Save share file")
                .set_file_name(&file_name)
                .add_filter("Arctic share", &["json"])
                .save_file();
            let result = match picked {
                None => Ok(None),
                Some(path) => std::fs::write(&path, bundle.to_file_text())
                    .map(|()| Some(path.clone()))
                    .map_err(|e| format!("Could not save {}: {e}", path.display())),
            };
            t.send(Event::ShareSaved(result));
        });
    }

    /// Ask for a share file and read it.
    pub fn share_open_file(&self) {
        self.run(|t| {
            let picked = rfd::FileDialog::new()
                .set_title("Open share file")
                .add_filter("Arctic share", &["json", "txt"])
                .pick_file();
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
                title: format!("Added {}", instance.name),
                detail: format!(
                    "{} mods installed. Press Play when you're ready.",
                    report.installed.len()
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
                title: format!("Applied to {}", target.name),
                detail: "You'll see it next time you play.".into(),
                warnings: Vec::new(),
                open: None,
                settings_changed: false,
            })
        }
        Bundle::Profile(pack) => {
            let report = pack.import(dirs, progress)?;
            let mut warnings = skipped(&report.skipped_mods);
            warnings.extend(
                report
                    .existing
                    .iter()
                    .map(|n| format!("Kept your own \"{n}\" (same name)")),
            );
            Ok(Imported {
                title: "Profile imported".into(),
                detail: format!(
                    "Settings applied, {} instance(s) added.",
                    report.created.len()
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
        .map(|s| format!("Skipped {}: {}", s.title, s.reason))
        .collect()
}
