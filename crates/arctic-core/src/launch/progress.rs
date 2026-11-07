//! Overall progress for one launch.
//!
//! Downloads report their own byte ratio. The Play button used to show that
//! ratio for whichever stage was running, and rounded it to the nearest
//! percent, so 301.4 MB of 302.7 MB (and the same near the end of "Downloading
//! mods") read as 100% while later work was still going. Libraries, assets and
//! the client jar share the "Downloading game files" stage; the loader install,
//! mods and the loader's own libraries are later stages. This tracker weights
//! them and stays at 99 until [`LaunchProgress::finish`].

use crate::instances::{Instance, Loader};

/// How much of the launch `prepare` still has to do, from the instance's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchSteps {
    /// A loader profile is installed (Fabric/Quilt/Forge/NeoForge, or Vanilla
    /// running on Fabric for the client and performance mods).
    pub loader: bool,
    /// Performance mods, shaders or extras may be downloaded.
    pub mods: bool,
    /// Forge or NeoForge: the installer jar, its libraries and patching.
    pub forge: bool,
}

impl LaunchSteps {
    pub fn from_instance(instance: &Instance) -> Self {
        let explicit = instance.loader.kind().is_some();
        // Vanilla with the client or the performance mods launches on Fabric.
        let loader = explicit || instance.arctic_mod || instance.performance || instance.shaders;
        let forge = matches!(
            instance.loader,
            Loader::Forge { .. } | Loader::NeoForge { .. }
        );
        let mods = instance.performance || instance.shaders || !instance.extras.is_empty();
        Self {
            loader,
            mods,
            forge,
        }
    }
}

/// Relative weights. The vanilla download (client, libraries, assets, Java)
/// is most of the bytes; the loader's libraries and mods are the rest.
const GAME_FILES: u32 = 58;
const LOADER_FILES: u32 = 16;
const MODS: u32 = 18;
const LOADER: u32 = 8;
const INSTALLER: u32 = 6;
const PATCH: u32 = 8;
const JAVA: u32 = 12;

/// Basis points: 10_000 is a finished stage. Anything short of finished is at most 9_999,
/// so a ratio of 99.57% cannot round up to "done".
const DONE_BPS: u32 = 10_000;
const ALMOST_BPS: u32 = DONE_BPS - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    GameFiles,
    Mods,
    Loader,
    Installer,
    Patch,
    Java,
    Light,
}

fn kind_of(stage: &str) -> Kind {
    if stage == "Downloading game files" {
        Kind::GameFiles
    } else if stage == "Downloading mods" {
        Kind::Mods
    } else if stage == "Java runtime" {
        Kind::Java
    } else if stage.starts_with("Installing ") {
        Kind::Loader
    } else if stage.starts_with("Patching ") {
        Kind::Patch
    } else if stage.starts_with("Downloading ") {
        Kind::Installer
    } else {
        Kind::Light
    }
}

/// 0..=10_000 for one stage's own counters. 10_000 only when every counted unit is finished.
fn stage_bps(stage: &str, done: u64, total: u64, bytes_done: u64, bytes_total: u64) -> u32 {
    if total == 0 && bytes_total == 0 {
        // A download with nothing queued is finished. A named step ("Checking files") is not.
        return if is_download(stage) { DONE_BPS } else { 0 };
    }
    let files_done = total == 0 || done >= total;
    let bytes_done_all = bytes_total == 0 || bytes_done >= bytes_total;
    if files_done && bytes_done_all {
        return DONE_BPS;
    }
    let mut bps = DONE_BPS;
    if bytes_total > 0 {
        bps = bps.min(ratio_bps(bytes_done, bytes_total));
    }
    if total > 0 {
        bps = bps.min(ratio_bps(done, total));
    }
    bps.min(ALMOST_BPS)
}

fn is_download(stage: &str) -> bool {
    stage.starts_with("Downloading ") || stage == "Java runtime"
}

fn ratio_bps(done: u64, total: u64) -> u32 {
    if total == 0 {
        return 0;
    }
    let bps = (u128::from(done) * u128::from(DONE_BPS) / u128::from(total)) as u32;
    bps.min(DONE_BPS)
}

/// Running total of a launch. [`observe`](Self::observe) is at most 99; [`finish`](Self::finish) is 100.
#[derive(Debug, Clone)]
pub struct LaunchProgress {
    finished: u32,
    current_weight: u32,
    current_bps: u32,
    stage: Option<String>,
    kind: Option<Kind>,
    game_files_left: u8,
    game_files_seen: u8,
    mods_left: u8,
    loader_left: u8,
    installer_left: u8,
    patch_left: u8,
    java_left: u8,
}

impl LaunchProgress {
    pub fn new(steps: LaunchSteps) -> Self {
        Self {
            finished: 0,
            current_weight: 0,
            current_bps: 0,
            stage: None,
            kind: None,
            game_files_left: if steps.loader { 2 } else { 1 },
            game_files_seen: 0,
            mods_left: u8::from(steps.mods),
            loader_left: u8::from(steps.loader),
            installer_left: if steps.forge { 3 } else { 0 },
            patch_left: u8::from(steps.forge),
            java_left: 0,
        }
    }

    /// Fold in one progress snapshot. The same stage updating in place does not
    /// start a new slice; "Patching Minecraft (1/n)" through "(n/n)" share one.
    pub fn observe(
        &mut self,
        stage: &str,
        done: u64,
        total: u64,
        bytes_done: u64,
        bytes_total: u64,
    ) -> u8 {
        let kind = kind_of(stage);
        if self.stage.as_deref() != Some(stage) {
            let same_patch = self.kind == Some(Kind::Patch) && kind == Kind::Patch;
            if same_patch {
                self.stage = Some(stage.to_owned());
            } else {
                self.commit();
                self.begin(stage, kind);
            }
        }
        self.current_bps = stage_bps(stage, done, total, bytes_done, bytes_total);
        self.percent()
    }

    /// The pipeline returned. This is the only time the value is 100.
    pub fn finish(&mut self) -> u8 {
        100
    }

    fn begin(&mut self, stage: &str, kind: Kind) {
        self.current_weight = self.take(kind);
        self.current_bps = 0;
        self.kind = Some(kind);
        self.stage = Some(stage.to_owned());
    }

    fn commit(&mut self) {
        self.finished = self.finished.saturating_add(self.current_weight);
        self.current_weight = 0;
        self.current_bps = 0;
    }

    fn take(&mut self, kind: Kind) -> u32 {
        match kind {
            Kind::GameFiles => self.take_game_files(),
            Kind::Mods => Self::take_one(&mut self.mods_left, MODS),
            Kind::Loader => Self::take_one(&mut self.loader_left, LOADER),
            Kind::Installer => Self::take_one(&mut self.installer_left, INSTALLER),
            Kind::Patch => Self::take_one(&mut self.patch_left, PATCH),
            Kind::Java => Self::take_one(&mut self.java_left, JAVA),
            Kind::Light => 0,
        }
    }

    fn take_game_files(&mut self) -> u32 {
        let weight = if self.game_files_seen == 0 {
            GAME_FILES
        } else {
            LOADER_FILES
        };
        self.game_files_seen = self.game_files_seen.saturating_add(1);
        if self.game_files_left > 0 {
            self.game_files_left -= 1;
        }
        weight
    }

    fn take_one(left: &mut u8, weight: u32) -> u32 {
        if *left > 0 {
            *left -= 1;
        }
        weight
    }

    fn pending(&self) -> u32 {
        self.game_files_pending()
            + u32::from(self.mods_left) * MODS
            + u32::from(self.loader_left) * LOADER
            + u32::from(self.installer_left) * INSTALLER
            + u32::from(self.patch_left) * PATCH
            + u32::from(self.java_left) * JAVA
    }

    fn game_files_pending(&self) -> u32 {
        let mut weight = 0;
        let mut seen = self.game_files_seen;
        for _ in 0..self.game_files_left {
            weight += if seen == 0 { GAME_FILES } else { LOADER_FILES };
            seen = seen.saturating_add(1);
        }
        weight
    }

    fn percent(&self) -> u8 {
        let denom = self.finished + self.current_weight + self.pending();
        if denom == 0 {
            return 0;
        }
        let numer = u64::from(self.finished) * u64::from(DONE_BPS)
            + u64::from(self.current_weight) * u64::from(self.current_bps);
        let pct = numer / (u64::from(denom) * 100);
        pct.min(99) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modded() -> LaunchProgress {
        LaunchProgress::new(LaunchSteps {
            loader: true,
            mods: true,
            forge: false,
        })
    }

    #[test]
    fn near_the_end_of_game_files_does_not_round_up_to_100() {
        // 301.4 / 302.7 ≈ 99.57%, which `{:.0}` renders as 100.
        let bytes_done = 301_400_000u64;
        let bytes_total = 302_700_000u64;
        assert_eq!(
            format!("{:.0}", bytes_done as f64 / bytes_total as f64 * 100.0),
            "100"
        );

        let mut vanilla = LaunchProgress::new(LaunchSteps {
            loader: false,
            mods: false,
            forge: false,
        });
        let alone = vanilla.observe(
            "Downloading game files",
            5127,
            5132,
            bytes_done,
            bytes_total,
        );
        assert_eq!(alone, 99);
        assert_eq!(vanilla.finish(), 100);

        let mut progress = modded();
        progress.observe("Reading version", 0, 0, 0, 0);
        progress.observe("Checking files", 0, 0, 0, 0);
        let during_files = progress.observe(
            "Downloading game files",
            5127,
            5132,
            bytes_done,
            bytes_total,
        );
        // Mods, the loader and its libraries are still ahead, so this is not "almost everything".
        assert_eq!(during_files, 57);
    }

    #[test]
    fn mods_and_a_byte_count_past_the_total_stay_under_100() {
        let mut progress = modded();
        progress.observe("Downloading game files", 10, 10, 100, 100);
        progress.observe("Preparing", 0, 0, 0, 0);
        progress.observe("Installing Fabric 0.20.1", 0, 0, 0, 0);
        progress.observe("Checking performance mods", 0, 0, 0, 0);
        // Bytes are already past the expected size; files are not done.
        let during_mods = progress.observe("Downloading mods", 7, 8, 2_000, 1_000);
        assert!(during_mods < 100, "{during_mods}");
        assert!(during_mods > 57, "{during_mods}");
        assert_eq!(progress.finish(), 100);
    }

    #[test]
    fn finished_bytes_with_files_left_are_not_done() {
        let mut progress = LaunchProgress::new(LaunchSteps {
            loader: false,
            mods: false,
            forge: false,
        });
        let pct = progress.observe("Downloading game files", 5127, 5132, 1_000, 1_000);
        assert_eq!(pct, 99);
    }

    #[test]
    fn the_same_stage_updates_in_place() {
        let mut progress = modded();
        let early = progress.observe("Downloading game files", 1_000, 2_000, 100, 1_000);
        let later = progress.observe("Downloading game files", 1_500, 2_000, 500, 1_000);
        assert!(later > early, "{early} -> {later}");
        assert_eq!(
            later,
            progress.observe("Downloading game files", 1_500, 2_000, 500, 1_000)
        );
    }

    #[test]
    fn patching_steps_share_one_slice() {
        let mut progress = LaunchProgress::new(LaunchSteps {
            loader: true,
            mods: false,
            forge: true,
        });
        let first = progress.observe("Patching Minecraft (1/4)", 0, 4, 0, 0);
        let third = progress.observe("Patching Minecraft (3/4)", 2, 4, 0, 0);
        assert!(third > first, "{first} -> {third}");
        assert!(third < 100);
    }

    #[test]
    fn steps_follow_the_instance() {
        let vanilla = Instance::vanilla_default();
        let steps = LaunchSteps::from_instance(&vanilla);
        assert!(steps.loader && steps.mods && !steps.forge);

        let mut pure = Instance::vanilla_default();
        pure.arctic_mod = false;
        pure.performance = false;
        pure.shaders = false;
        let steps = LaunchSteps::from_instance(&pure);
        assert!(!steps.loader && !steps.mods && !steps.forge);

        let mut forge = Instance::vanilla_default();
        forge.loader = Loader::Forge {
            version: "47.4.0".into(),
        };
        forge.performance = true;
        let steps = LaunchSteps::from_instance(&forge);
        assert!(steps.loader && steps.mods && steps.forge);
    }
}
