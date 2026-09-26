//! Modrinth App: instances in its SQLite database (the `instances` tables
//! since mid-2026, `profiles` before that), and the older Theseus
//! `profile.json` files.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

use super::{app_data, found, loader, memory, read_json, subdirs};
use crate::migrate::{Found, Launcher, Scan};

struct Row {
    path: String,
    name: String,
    game: String,
    loader: String,
    loader_version: Option<String>,
    memory: Option<u64>,
}

pub fn app(scan: &mut Scan) {
    let Some(base) = app_data() else {
        return;
    };
    let root = base.join("ModrinthApp");
    let db = root.join("app.db");
    if db.is_file() {
        match read_db(&db) {
            Ok((rows, custom_dir)) => {
                let profiles = custom_dir.unwrap_or(root).join("profiles");
                let before = scan.instances.len();
                for row in rows {
                    scan.instances.push(entry(row, &profiles));
                }
                if scan.instances.len() == before {
                    scan.seen
                        .push((Launcher::ModrinthApp, "No instances".into()));
                }
            }
            Err(e) => scan.seen.push((
                Launcher::ModrinthApp,
                format!("Couldn't read its database: {e}"),
            )),
        }
    }
    let legacy = base.join("com.modrinth.theseus").join("profiles");
    for dir in subdirs(&legacy) {
        if let Some(f) = legacy_profile(&dir) {
            scan.instances.push(f);
        }
    }
}

fn entry(row: Row, profiles: &Path) -> Found {
    let found_loader = loader(&row.loader, row.loader_version.as_deref(), &row.game);
    let mut f = found(
        Launcher::ModrinthApp,
        &row.name,
        &row.game,
        found_loader,
        profiles.join(&row.path),
    );
    f.memory_mb = memory(row.memory);
    f
}

/// Instances, and the custom data folder if one is set.
fn read_db(db: &Path) -> rusqlite::Result<(Vec<Row>, Option<PathBuf>)> {
    // Read-only, so a running Modrinth App is never disturbed.
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let has = |table: &str| -> rusqlite::Result<bool> {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
    };
    let rows = if has("instances")? && has("instance_content_sets")? {
        let mut stmt = conn.prepare(
            "SELECT i.path, i.name, c.game_version, c.loader, c.loader_version, json(o.overrides)
             FROM instances i
             JOIN instance_content_sets c ON c.id = i.applied_content_set_id
             LEFT JOIN instance_launch_overrides o ON o.instance_id = i.id",
        )?;
        stmt.query_map([], |r| {
            let overrides: Option<String> = r.get(5)?;
            Ok(Row {
                path: r.get(0)?,
                name: r.get(1)?,
                game: r.get(2)?,
                loader: r.get(3)?,
                loader_version: r.get(4)?,
                memory: overrides
                    .and_then(|o| serde_json::from_str::<Value>(&o).ok())
                    .and_then(|o| o.pointer("/memory/maximum").and_then(Value::as_u64)),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else if has("profiles")? {
        let mut stmt = conn.prepare(
            "SELECT path, name, game_version, mod_loader, mod_loader_version, override_mc_memory_max FROM profiles",
        )?;
        stmt.query_map([], |r| {
            Ok(Row {
                path: r.get(0)?,
                name: r.get(1)?,
                game: r.get(2)?,
                loader: r.get(3)?,
                loader_version: r.get(4)?,
                memory: r.get::<_, Option<i64>>(5)?.map(|m| m.max(0) as u64),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        Vec::new()
    };
    let custom: Option<String> = if has("settings")? {
        conn.query_row("SELECT custom_dir FROM settings LIMIT 1", [], |r| r.get(0))
            .optional()?
            .flatten()
    } else {
        None
    };
    Ok((rows, custom.filter(|d| !d.is_empty()).map(PathBuf::from)))
}

/// An old Theseus profile folder (`profile.json`).
pub fn legacy_profile(dir: &Path) -> Option<Found> {
    let json = read_json(&dir.join("profile.json"))?;
    let meta = json.get("metadata")?;
    let game = meta.get("game_version").and_then(Value::as_str)?.to_owned();
    let name = meta
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Modrinth profile");
    let kind = meta
        .get("loader")
        .and_then(Value::as_str)
        .unwrap_or("vanilla");
    let version = meta.pointer("/loader_version/id").and_then(Value::as_str);
    let mut f = found(
        Launcher::ModrinthApp,
        name,
        &game,
        loader(kind, version, &game),
        dir.to_path_buf(),
    );
    f.memory_mb = memory(json.pointer("/memory/maximum").and_then(Value::as_u64));
    Some(f)
}
