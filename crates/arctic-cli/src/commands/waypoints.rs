//! `arctic waypoints …`: Arctic Client waypoints of an instance.

use arctic_core::waypoints;
use arctic_core::{Error, Result};
use serde_json::json;

use super::{Ctx, instance_or_default};
use crate::cli::WaypointsCommand;

pub fn run(ctx: &Ctx, command: &WaypointsCommand) -> Result<i32> {
    let instance = instance_or_default(ctx, command.instance())?;
    let game_dir = instance.game_dir(&ctx.dirs);
    let mut all = waypoints::load(&game_dir)?;
    match command {
        WaypointsCommand::List { world, .. } => {
            for (key, list) in &all {
                if world.as_deref().is_some_and(|w| w != key) {
                    continue;
                }
                for w in list {
                    ctx.out.emit(
                        json!({"world": key, "name": w.name, "x": w.x, "y": w.y, "z": w.z, "dim": w.dim, "shown": w.shown}),
                        || format!("{key:<28} {:<20} {:>7} {:>4} {:>7}  {}", w.name, w.x, w.y, w.z, w.dim),
                    );
                }
            }
        }
        WaypointsCommand::Add {
            name,
            x,
            y,
            z,
            world,
            dim,
            ..
        } => {
            waypoints::add(
                &mut all,
                world,
                waypoints::new(name, *x, *y, *z, dim.as_deref()),
            )?;
            waypoints::save(&game_dir, &all)?;
            ctx.out.emit(json!({"added": name, "world": world}), || {
                format!("Added {name} to {world}.")
            });
        }
        WaypointsCommand::Remove { name, world, .. } => {
            if waypoints::remove(&mut all, world, name) == 0 {
                return Err(Error::Other(format!("{world} has no waypoint \"{name}\"")));
            }
            waypoints::save(&game_dir, &all)?;
            ctx.out.emit(json!({"removed": name, "world": world}), || {
                format!("Removed {name} from {world}.")
            });
        }
    }
    Ok(0)
}
