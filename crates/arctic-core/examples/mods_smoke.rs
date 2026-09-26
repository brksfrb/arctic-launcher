//! Live smoke test of the Modrinth mod browser against the real API.
//!
//! ```text
//! cargo run -p arctic-core --example mods_smoke
//! ```
//! Searches, installs Sodium Extra (pulling in Sodium), Fabric API and Sodium
//! for Fabric 1.21.4 into a temp folder, lists, disables and removes.
//! Nothing is launched.

use arctic_core::ProgressInfo;
use arctic_core::loaders::LoaderKind;
use arctic_core::mods::{self, SearchQuery, SortBy};

const GAME: &str = "1.21.4";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let temp = tempfile::tempdir()?;
    let instance = temp.path();
    let mods_dir = instance.join("minecraft").join("mods");
    let index = mods::index_path(instance);

    let page = mods::search(&SearchQuery {
        text: "sodium".into(),
        game_version: GAME.into(),
        loader: Some(LoaderKind::Fabric),
        sort: SortBy::Relevance,
        offset: 0,
        limit: 5,
        modpacks: false,
    })?;
    println!("search: {} total hits", page.total);
    for hit in &page.hits {
        println!(
            "  {:<24} {:<10} {:>12} downloads  by {}",
            hit.title, hit.project_id, hit.downloads, hit.author
        );
    }

    let progress = |p: ProgressInfo| {
        if p.total > 0 && p.done == p.total {
            println!(
                "  {}: {}/{} files, {} bytes",
                p.stage, p.done, p.total, p.bytes_done
            );
        }
    };
    // Sodium Extra requires Sodium, so Sodium first arrives as a dependency;
    // installing it explicitly afterwards re-records it as a direct install.
    for project in ["sodium-extra", "fabric-api", "sodium"] {
        let installed = mods::install(
            project,
            GAME,
            LoaderKind::Fabric,
            &mods_dir,
            &index,
            &progress,
        )?;
        println!("installed {project}:");
        for m in &installed {
            let dep = if m.dependency { " (dependency)" } else { "" };
            println!("  {} {} -> {}{dep}", m.title, m.version_number, m.file_name);
        }
    }

    print_list("after install", &mods_dir, &index)?;
    let first = mods::list(&mods_dir, &index)?
        .into_iter()
        .next()
        .ok_or("nothing installed")?;
    mods::set_enabled(&mods_dir, &first.file_name, false)?;
    print_list("after disabling the first", &mods_dir, &index)?;
    mods::remove(&mods_dir, &index, &first.file_name)?;
    print_list("after removing it", &mods_dir, &index)?;

    if let Some(url) = page.hits.first().and_then(|h| h.icon_url.clone()) {
        let cache = instance.join("icons");
        let bytes = mods::icon(&url, &cache)?;
        let again = mods::icon(&url, &cache)?;
        println!(
            "icon: {} bytes (cached copy equal: {})",
            bytes.len(),
            bytes == again
        );
    }
    Ok(())
}

fn print_list(
    label: &str,
    mods_dir: &std::path::Path,
    index: &std::path::Path,
) -> arctic_core::Result<()> {
    println!("list {label}:");
    for m in mods::list(mods_dir, index)? {
        let title = m
            .tracked
            .as_ref()
            .map_or("(untracked)", |t| t.title.as_str());
        let state = if m.enabled { "on " } else { "off" };
        let dep = if m.tracked.as_ref().is_some_and(|t| t.dependency) {
            " [dependency]"
        } else {
            ""
        };
        println!(
            "  [{state}] {title:<20} {} ({} bytes){dep}",
            m.file_name, m.size
        );
    }
    Ok(())
}
