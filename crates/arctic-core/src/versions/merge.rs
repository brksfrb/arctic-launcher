//! Layering a mod-loader profile (`inheritsFrom`) on top of its vanilla
//! parent, the way the official launcher does.

use std::collections::HashSet;

use super::model::{Arguments, Library, VersionJson};

/// Merge `child` (a loader profile) onto `parent` (the vanilla version).
///
/// - `id`, `mainClass`: from the child.
/// - Arguments: parent's first, then the child's (both `game` and `jvm`).
/// - Legacy `minecraftArguments`: the child's replaces the parent's.
/// - Assets, downloads, Java version, logging: the child's if present.
/// - Libraries: the child's first; parent libraries with the same
///   `group:artifact[:classifier]` are dropped (the loader pins its own).
pub fn merge(parent: VersionJson, child: VersionJson) -> VersionJson {
    let arguments = match (parent.arguments, child.arguments) {
        (None, None) => None,
        (p, c) => {
            let (p, c) = (p.unwrap_or_default(), c.unwrap_or_default());
            Some(Arguments {
                game: p.game.into_iter().chain(c.game).collect(),
                jvm: p.jvm.into_iter().chain(c.jvm).collect(),
            })
        }
    };
    let child_keys: HashSet<String> = child.libraries.iter().map(library_key).collect();
    let libraries = child
        .libraries
        .into_iter()
        .chain(
            parent
                .libraries
                .into_iter()
                .filter(|l| !child_keys.contains(&library_key(l))),
        )
        .collect();
    VersionJson {
        id: child.id,
        kind: if child.kind.is_empty() {
            parent.kind
        } else {
            child.kind
        },
        main_class: child.main_class,
        arguments,
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        libraries,
        java_version: child.java_version.or(parent.java_version),
        logging: child.logging.or(parent.logging),
        inherits_from: None,
    }
}

/// `group:artifact[:classifier]`: a library's identity without its version.
fn library_key(lib: &Library) -> String {
    let parts: Vec<&str> = lib
        .name
        .split('@')
        .next()
        .unwrap_or("")
        .split(':')
        .collect();
    match parts.as_slice() {
        [group, artifact, _version, classifier, ..] => format!("{group}:{artifact}:{classifier}"),
        [group, artifact, ..] => format!("{group}:{artifact}"),
        _ => lib.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> VersionJson {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn loader_profile_overrides_and_appends() {
        let parent = parse(
            r#"{"id":"1.21","type":"release","mainClass":"net.minecraft.client.main.Main",
                "arguments":{"game":["--username","x"],"jvm":["-cp","${classpath}"]},
                "assetIndex":{"id":"17","sha1":"s","size":1,"url":"u"},
                "javaVersion":{"component":"java-runtime-delta","majorVersion":21},
                "libraries":[{"name":"org.ow2.asm:asm:9.3"},{"name":"com.mojang:brigadier:1.0"}]}"#,
        );
        let child = parse(
            r#"{"id":"fabric-loader-0.16-1.21","inheritsFrom":"1.21","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
                "arguments":{"game":[],"jvm":["-DFabricMcEmu= net.minecraft.client.main.Main "]},
                "libraries":[{"name":"org.ow2.asm:asm:9.7","url":"https://maven.fabricmc.net/"},
                             {"name":"net.fabricmc:fabric-loader:0.16","url":"https://maven.fabricmc.net/"}]}"#,
        );
        let merged = merge(parent, child);
        assert_eq!(merged.id, "fabric-loader-0.16-1.21");
        assert_eq!(merged.kind, "release");
        assert!(merged.main_class.contains("KnotClient"));
        assert!(merged.inherits_from.is_none());
        assert_eq!(merged.java_version.unwrap().major_version, 21);
        assert_eq!(merged.asset_index.unwrap().id, "17");
        let names: Vec<&str> = merged.libraries.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "org.ow2.asm:asm:9.7",
                "net.fabricmc:fabric-loader:0.16",
                "com.mojang:brigadier:1.0"
            ]
        );
        let args = merged.arguments.unwrap();
        assert_eq!(args.jvm.len(), 3);
        assert_eq!(args.game.len(), 2);
    }

    #[test]
    fn legacy_forge_replaces_minecraft_arguments() {
        let parent =
            parse(r#"{"id":"1.12.2","mainClass":"M","minecraftArguments":"--username a"}"#);
        let child = parse(
            r#"{"id":"1.12.2-forge","mainClass":"net.minecraft.launchwrapper.Launch",
                "minecraftArguments":"--username a --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker"}"#,
        );
        let merged = merge(parent, child);
        assert!(merged.minecraft_arguments.unwrap().contains("FMLTweaker"));
        assert!(merged.arguments.is_none());
    }

    #[test]
    fn classifier_is_part_of_identity() {
        let lib = |n: &str| Library {
            name: n.into(),
            downloads: None,
            url: None,
            sha1: None,
            size: None,
            natives: None,
            rules: None,
            extract: None,
        };
        assert_eq!(
            library_key(&lib("a:b:1:natives-windows")),
            "a:b:natives-windows"
        );
        assert_eq!(library_key(&lib("a:b:1@zip")), "a:b");
    }
}
