//! `arctic instances …`.

use arctic_core::instances::{self, FlakeStyle, Instance, Loader};
use arctic_core::loaders::{self, LoaderKind};
use arctic_core::versions::VersionManifest;
use arctic_core::{Error, Result};
use serde_json::json;

use super::{Ctx, resolve_version};
use crate::cli::{IconArg, InstanceEditArgs, InstancesCommand, LoaderArg};

pub fn run(ctx: &Ctx, command: &InstancesCommand) -> Result<i32> {
    match command {
        InstancesCommand::List => list(ctx)?,
        InstancesCommand::Create {
            name,
            version,
            loader,
            loader_version,
        } => create(ctx, name, version, *loader, loader_version.as_deref())?,
        InstancesCommand::Show { instance } => show(ctx, &instances::find(&ctx.dirs, instance)?),
        InstancesCommand::Edit(args) => edit(ctx, args)?,
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

fn show(ctx: &Ctx, i: &Instance) {
    let memory = i.max_memory_mb.map(|mb| format!("{mb} MB"));
    let java = i.java_path.as_ref().map(|p| p.display().to_string());
    let color = i
        .icon
        .color
        .map(|[r, g, b]| format!("#{r:02x}{g:02x}{b:02x}"));
    ctx.out.emit(
        json!({
            "id": i.id,
            "name": i.name,
            "version": i.version,
            "loader": i.loader.label(),
            "loader_version": i.loader.version(),
            "memory_mb": i.max_memory_mb,
            "icon": i.icon.style.label().to_lowercase(),
            "color": color,
            "arctic_client": i.arctic_mod,
            "java": java,
            "jvm_args": i.jvm_args,
            "performance": i.performance,
            "shaders": i.shaders,
            "game_dir": i.game_dir(&ctx.dirs),
        }),
        || {
            let on = |b: bool| if b { "on" } else { "off" };
            let mut lines = vec![
                format!("{} ({})", i.name, i.id),
                format!(
                    "  version        {}",
                    i.version.as_deref().unwrap_or("any (follows Play)")
                ),
                format!(
                    "  loader         {} {}",
                    i.loader.label(),
                    i.loader.version().unwrap_or("")
                ),
                format!(
                    "  memory         {}",
                    memory.as_deref().unwrap_or("launcher default")
                ),
                format!(
                    "  icon           {} {}",
                    i.icon.style.label(),
                    color.as_deref().unwrap_or("")
                ),
                format!(
                    "  java           {}",
                    java.as_deref().unwrap_or("launcher default")
                ),
                format!("  jvm args       {}", i.jvm_args),
                format!("  game folder    {}", i.game_dir(&ctx.dirs).display()),
            ];
            if i.loader.kind().is_some() {
                lines.push(format!("  arctic client  {}", on(i.arctic_mod)));
            } else {
                lines.push(format!("  performance    {}", on(i.performance)));
                lines.push(format!("  shaders        {}", on(i.shaders)));
            }
            lines.join("\n")
        },
    );
}

fn edit(ctx: &Ctx, args: &InstanceEditArgs) -> Result<()> {
    let mut i = instances::find(&ctx.dirs, &args.instance)?;
    if let Some(name) = &args.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Other("the name can't be empty".into()));
        }
        i.name = name.to_owned();
    }
    let game_changes =
        args.version.is_some() || args.loader.is_some() || args.loader_version.is_some();
    if game_changes {
        if i.is_default() {
            return Err(Error::Other(
                "the Vanilla instance follows the version picked to play; make another instance to pin one".into(),
            ));
        }
        let game = match &args.version {
            Some(v) => resolve_version(&VersionManifest::fetch(&ctx.dirs)?, v)?.id,
            None => i.version.clone().unwrap_or_default(),
        };
        let kind = match args.loader {
            Some(arg) => kind(arg),
            None => i.loader.kind(),
        };
        let loader_version = match (kind, &args.loader_version) {
            (None, _) => String::new(),
            (Some(_), Some(v)) => v.clone(),
            // Same loader and version: keep its build.
            (Some(k), None) if Some(k) == i.loader.kind() && Some(&game) == i.version.as_ref() => {
                i.loader.version().unwrap_or_default().to_owned()
            }
            (Some(k), None) => default_loader_version(k, &game)?,
        };
        i.version = Some(game);
        i.loader = Loader::new(kind, loader_version);
    }
    if let Some(memory) = &args.memory {
        i.max_memory_mb = match memory.as_str() {
            "default" => None,
            text => Some(crate::output::parse_memory(text).ok_or_else(|| {
                Error::Other(format!("'{text}' isn't a memory size (like 4G or 6144M)"))
            })?),
        };
    }
    if let Some(icon) = args.icon {
        i.icon.style = style(icon);
    }
    if let Some(color) = &args.color {
        i.icon.color = match color.as_str() {
            "default" => None,
            text => Some(
                hex_color(text)
                    .ok_or_else(|| Error::Other(format!("'{text}' isn't a color like #5ec8f2")))?,
            ),
        };
    }
    if let Some(on) = args.arctic_client {
        i.arctic_mod = on.on();
    }
    if let Some(java) = &args.java {
        i.java_path = match java.as_str() {
            "default" => None,
            path => {
                let path = std::path::PathBuf::from(path);
                if !path.is_file() {
                    return Err(Error::Other(format!("no Java at {}", path.display())));
                }
                Some(path)
            }
        };
    }
    if let Some(flags) = &args.jvm_args {
        i.jvm_args = flags.trim().to_owned();
    }
    if let Some(on) = args.performance {
        i.performance = on.on();
    }
    if let Some(on) = args.shaders {
        i.shaders = on.on();
    }
    i.save(&ctx.dirs)?;
    show(ctx, &i);
    Ok(())
}

fn style(icon: IconArg) -> FlakeStyle {
    match icon {
        IconArg::Classic => FlakeStyle::Classic,
        IconArg::Stellar => FlakeStyle::Stellar,
        IconArg::Dendrite => FlakeStyle::Dendrite,
        IconArg::Plate => FlakeStyle::Plate,
        IconArg::Star => FlakeStyle::Star,
        IconArg::Crystal => FlakeStyle::Crystal,
    }
}

/// `#rrggbb` (or without the `#`).
fn hex_color(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#').unwrap_or(text);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hex_colors() {
        assert_eq!(hex_color("#5ec8f2"), Some([0x5e, 0xc8, 0xf2]));
        assert_eq!(hex_color("FFFFFF"), Some([255, 255, 255]));
        assert_eq!(hex_color("#fff"), None);
        assert_eq!(hex_color("#zzzzzz"), None);
    }
}
