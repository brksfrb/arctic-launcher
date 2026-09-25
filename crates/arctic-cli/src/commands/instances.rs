//! `arctic instances …`.

use arctic_core::instances::{self, Loader};
use arctic_core::loaders::{self, LoaderKind};
use arctic_core::versions::VersionManifest;
use arctic_core::{Error, Result};
use serde_json::json;

use super::{Ctx, resolve_version};
use crate::cli::{InstancesCommand, LoaderArg};

pub fn run(ctx: &Ctx, command: &InstancesCommand) -> Result<i32> {
    match command {
        InstancesCommand::List => list(ctx)?,
        InstancesCommand::Create {
            name,
            version,
            loader,
            loader_version,
        } => create(ctx, name, version, *loader, loader_version.as_deref())?,
        InstancesCommand::Remove { instance } => {
            let found = instances::find(&ctx.dirs, instance)?;
            if found.is_default() {
                return Err(Error::Other("the Vanilla instance can't be removed".into()));
            }
            instances::remove(&ctx.dirs, &found.id)?;
            ctx.out
                .emit(json!({"event": "removed", "id": found.id}), || {
                    format!("Moved {} to instances/.trash", found.name)
                });
        }
    }
    Ok(0)
}

fn list(ctx: &Ctx) -> Result<()> {
    let mut all = vec![instances::load_default(&ctx.dirs)?];
    all.extend(instances::list_custom(&ctx.dirs)?);
    for i in all {
        let version = i.version.clone().unwrap_or_else(|| "any".into());
        let loader = match i.loader.version() {
            Some(v) => format!("{} {v}", i.loader.label()),
            None => i.loader.label().to_owned(),
        };
        ctx.out.emit(
            json!({
                "id": i.id,
                "name": i.name,
                "version": i.version,
                "loader": i.loader.label(),
                "loader_version": i.loader.version(),
            }),
            || format!("{:<24} {:<20} {version:<10} {loader}", i.id, i.name),
        );
    }
    Ok(())
}

fn kind(arg: LoaderArg) -> Option<LoaderKind> {
    match arg {
        LoaderArg::Vanilla => None,
        LoaderArg::Fabric => Some(LoaderKind::Fabric),
        LoaderArg::Quilt => Some(LoaderKind::Quilt),
        LoaderArg::Neoforge => Some(LoaderKind::NeoForge),
        LoaderArg::Forge => Some(LoaderKind::Forge),
    }
}

/// Newest stable loader build for a Minecraft version.
fn default_loader_version(kind: LoaderKind, game: &str) -> Result<String> {
    let versions = loaders::loader_versions(kind, game)?;
    versions
        .iter()
        .find(|v| v.stable)
        .or(versions.first())
        .map(|v| v.id.clone())
        .ok_or_else(|| {
            Error::Other(format!(
                "{} has no build for Minecraft {game}",
                kind.label()
            ))
        })
}

fn create(
    ctx: &Ctx,
    name: &str,
    version: &str,
    loader: LoaderArg,
    loader_version: Option<&str>,
) -> Result<()> {
    let manifest = VersionManifest::fetch(&ctx.dirs)?;
    let game = resolve_version(&manifest, version)?.id;
    let kind = kind(loader);
    let loader_version = match (kind, loader_version) {
        (None, _) => String::new(),
        (Some(_), Some(v)) => v.to_owned(),
        (Some(k), None) => default_loader_version(k, &game)?,
    };
    let instance = instances::create(&ctx.dirs, name, &game, Loader::new(kind, loader_version))?;
    let label = match instance.loader.version() {
        Some(v) => format!("{} {v}", instance.loader.label()),
        None => instance.loader.label().to_owned(),
    };
    ctx.out.emit(
        json!({
            "event": "created",
            "id": instance.id,
            "version": game,
            "loader": instance.loader.label(),
            "loader_version": instance.loader.version(),
        }),
        || {
            format!(
                "Created {} ({label}, Minecraft {game}). Start it with: arctic launch --instance {}",
                instance.name, instance.id
            )
        },
    );
    Ok(())
}
