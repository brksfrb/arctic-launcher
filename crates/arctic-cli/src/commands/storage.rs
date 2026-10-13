//! `arctic storage`: where Arctic keeps its files, and moving them to another folder.

use std::path::{Path, PathBuf};

use arctic_core::storage::location::{self, Target};
use arctic_core::storage::{RootSource, root_source};
use arctic_core::{Error, Result};
use serde_json::json;

use super::Ctx;
use crate::cli::StorageCommand;

pub fn run(ctx: &Ctx, cmd: &StorageCommand) -> Result<i32> {
    match cmd {
        StorageCommand::Show => show(ctx),
        StorageCommand::Move { folder } => move_to(ctx, folder),
        StorageCommand::Use { folder } => use_folder(ctx, &absolute(folder)?),
    }
}

/// How the folder in use was chosen; `--data-dir` counts as the environment variable.
fn source(ctx: &Ctx) -> Result<RootSource> {
    let source = root_source()?;
    Ok(if source.root() == ctx.root.root() {
        source
    } else {
        RootSource::Env(ctx.root.root().to_path_buf())
    })
}

fn show(ctx: &Ctx) -> Result<i32> {
    let source = source(ctx)?;
    let (how, extra) = match &source {
        RootSource::Env(_) => ("ARCTIC_DATA_DIR or --data-dir", None),
        RootSource::Portable(_) => ("portable mode (portable.txt next to the launcher)", None),
        RootSource::Chosen(_) => ("picked in Settings → Storage", None),
        RootSource::ChosenMissing { chosen, .. } => (
            "the usual folder, because the picked one isn't there",
            Some(chosen),
        ),
        RootSource::Default(_) => ("the usual folder", None),
    };
    let root = source.root();
    ctx.out.emit(
        json!({"path": root, "how": how, "missing": extra}),
        || match extra {
            Some(missing) => format!(
                "{}\n  {how}: {} isn't available",
                root.display(),
                missing.display()
            ),
            None => format!("{}\n  {how}", root.display()),
        },
    );
    Ok(0)
}

fn absolute(folder: &str) -> Result<PathBuf> {
    let path = PathBuf::from(folder);
    if path.is_absolute() {
        return Ok(path);
    }
    std::env::current_dir()
        .map(|dir| dir.join(path))
        .map_err(|e| Error::Other(e.to_string()))
}

fn movable(ctx: &Ctx) -> Result<(PathBuf, PathBuf)> {
    let source = source(ctx)?;
    if !source.movable() {
        return Err(Error::Other(
            "the folder is set by ARCTIC_DATA_DIR/--data-dir or portable mode; change that instead"
                .into(),
        ));
    }
    Ok((location::default_root()?, source.root().to_path_buf()))
}

fn move_to(ctx: &Ctx, folder: &str) -> Result<i32> {
    let (default, from) = movable(ctx)?;
    let to = if folder == "default" {
        default.clone()
    } else {
        absolute(folder)?
    };
    if let Some(why) = location::check_move(&from, &to) {
        return Err(Error::Other(why));
    }
    match location::inspect(&to) {
        Target::Empty => {}
        Target::ArcticData => {
            return Err(Error::Other(format!(
                "{} already has Arctic's files: `arctic storage use` it instead",
                to.display()
            )));
        }
        Target::Other => {
            return Err(Error::Other(format!(
                "{} has other things in it: pick an empty folder, like {}",
                to.display(),
                to.join("Arctic Launcher").display()
            )));
        }
    }
    ctx.out.info(&format!(
        "Copying {} MB to {} (close the launcher first: changes made meanwhile stay behind)…",
        location::size_to_move(&from) / (1024 * 1024),
        to.display()
    ));
    location::move_data(&default, &from, &to, &|_, _| {})?;
    done(ctx, &to, Some(&from));
    Ok(0)
}

fn use_folder(ctx: &Ctx, to: &Path) -> Result<i32> {
    let (default, _) = movable(ctx)?;
    location::adopt(&default, to)?;
    done(ctx, to, None);
    Ok(0)
}

fn done(ctx: &Ctx, to: &Path, old: Option<&Path>) {
    ctx.out.emit(json!({"path": to, "old": old}), || match old {
        Some(old) => format!(
            "Arctic now keeps its files in {}. The old copy in {} is removed the next time the launcher starts.",
            to.display(),
            old.display()
        ),
        None => format!("Arctic now uses the files in {}.", to.display()),
    });
}
