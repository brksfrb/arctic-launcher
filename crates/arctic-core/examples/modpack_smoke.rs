//! Install a Modrinth modpack into a scratch data dir (no game launch):
//! `cargo run -p arctic-core --example modpack_smoke -- fabulously-optimized`
use std::time::Instant;

use arctic_core::mods::modpack;
use arctic_core::storage::DataDirs;

fn main() {
    let project = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "fabulously-optimized".into());
    let root = std::env::temp_dir().join("arctic-modpack-smoke");
    let _ = std::fs::remove_dir_all(&root);
    let dirs = DataDirs::new(&root).with_profile("default");
    dirs.ensure().unwrap();
    let t = Instant::now();
    let instance = modpack::install_from_modrinth(&dirs, &project, &|p| {
        if p.total > 0 && p.done == p.total {
            println!("  {}: {} files", p.stage, p.total);
        }
    })
    .unwrap();
    let mods = std::fs::read_dir(instance.game_dir(&dirs).join("mods"))
        .map(|d| d.count())
        .unwrap_or(0);
    let config = instance.game_dir(&dirs).join("config").is_dir();
    println!(
        "installed \"{}\" ({} {}, Minecraft {}) in {:.1}s: {mods} mods, config folder: {config}",
        instance.name,
        instance.loader.label(),
        instance.loader.version().unwrap_or(""),
        instance.version.as_deref().unwrap_or("?"),
        t.elapsed().as_secs_f32()
    );
}
