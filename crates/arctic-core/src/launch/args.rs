//! Pure command-line construction (no I/O), so it is easy to test.

use std::collections::HashMap;

use crate::versions::model::Argument;
use crate::versions::{RuleEnv, VersionJson, rules_allow};

/// Launcher-controlled JVM options that surround the version's own args.
#[derive(Debug, Clone, Default)]
pub struct JvmOptions {
    pub min_memory_mb: u32,
    pub max_memory_mb: u32,
    pub extra: Vec<String>,
    /// Already-substituted `-Dlog4j.configurationFile=...` if the version has one.
    pub logging_arg: Option<String>,
    pub fullscreen: bool,
    pub resolution: (u32, u32),
    /// Appended after the version's game arguments (e.g. `--proxyHost`).
    pub game_extra: Vec<String>,
}

/// Placeholder values for `${name}` substitution.
pub type Placeholders = HashMap<&'static str, String>;

pub(crate) const CUSTOM_MAX_HEAP_SOURCE: &str = " (maximum heap set by custom JVM arguments)";

/// Disable the Base64 decode intrinsic affected by Java4Shell (CVE-2026-114938).
pub(crate) fn java4shell_flags(
    major: Option<u32>,
    native_x86_64: bool,
    existing: &[String],
) -> &'static [&'static str] {
    if native_x86_64
        && major.is_some_and(|version| version >= 17)
        && !existing.iter().any(|arg| {
            matches!(
                arg.as_str(),
                "-XX:+UseBASE64Intrinsics" | "-XX:-UseBASE64Intrinsics"
            )
        })
    {
        &["-XX:+UnlockDiagnosticVMOptions", "-XX:-UseBASE64Intrinsics"]
    } else {
        &[]
    }
}

/// A Java heap size in MiB. Java interprets an unsuffixed size as bytes.
pub(crate) fn heap_size_mb(size: &str) -> Option<u32> {
    let end = size
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(size.len());
    let (number, unit) = size.split_at(end);
    let number = number.parse::<u64>().ok()?;
    let mib = match unit {
        "" => number / (1024 * 1024),
        "k" | "K" => number / 1024,
        "m" | "M" => number,
        "g" | "G" => number.checked_mul(1024)?,
        _ => return None,
    };
    mib.try_into().ok()
}

/// Arguments used by versions that predate the `arguments` object.
const LEGACY_JVM_ARGS: [&str; 3] = [
    "-Djava.library.path=${natives_directory}",
    "-cp",
    "${classpath}",
];

/// Build every argument after the java executable.
pub fn build(
    version: &VersionJson,
    env: &RuleEnv,
    vars: &Placeholders,
    opts: &JvmOptions,
) -> Vec<String> {
    let custom_min = opts.extra.iter().rposition(|arg| arg.starts_with("-Xms"));
    let custom_max = opts.extra.iter().rposition(|arg| arg.starts_with("-Xmx"));
    let mut args = Vec::new();
    if custom_min.is_none() {
        let min_mb = custom_max
            .and_then(|i| opts.extra[i].strip_prefix("-Xmx"))
            .and_then(heap_size_mb)
            .map_or(opts.min_memory_mb, |max| opts.min_memory_mb.min(max));
        args.push(format!("-Xms{min_mb}M"));
    }
    if custom_max.is_none() {
        args.push(format!("-Xmx{}M", opts.max_memory_mb));
    }
    // Keep the last explicit flag in its original position among the JVM arguments.
    args.extend(
        opts.extra
            .iter()
            .enumerate()
            .filter(|(i, arg)| match arg.as_str() {
                a if a.starts_with("-Xms") => Some(*i) == custom_min,
                a if a.starts_with("-Xmx") => Some(*i) == custom_max,
                _ => true,
            })
            .map(|(_, arg)| arg.clone()),
    );

    match &version.arguments {
        Some(a) if !a.jvm.is_empty() => {
            args.extend(expand(&a.jvm, env, vars).into_iter().filter(|arg| {
                (custom_min.is_none() || !arg.starts_with("-Xms"))
                    && (custom_max.is_none() || !arg.starts_with("-Xmx"))
            }));
        }
        _ => args.extend(LEGACY_JVM_ARGS.iter().map(|s| substitute(s, vars))),
    }
    args.extend(opts.logging_arg.iter().cloned());
    args.push(version.main_class.clone());

    match (&version.arguments, &version.minecraft_arguments) {
        (Some(a), _) if !a.game.is_empty() => args.extend(expand(&a.game, env, vars)),
        (_, Some(legacy)) => {
            args.extend(legacy.split_whitespace().map(|s| substitute(s, vars)));
            if !opts.fullscreen {
                let (w, h) = opts.resolution;
                args.extend([
                    "--width".into(),
                    w.to_string(),
                    "--height".into(),
                    h.to_string(),
                ]);
            }
        }
        _ => {}
    }
    if opts.fullscreen {
        args.push("--fullscreen".into());
    }
    args.extend(opts.game_extra.iter().cloned());
    args
}

fn expand(list: &[Argument], env: &RuleEnv, vars: &Placeholders) -> Vec<String> {
    list.iter()
        .flat_map(|arg| match arg {
            Argument::Plain(s) => vec![substitute(s, vars)],
            Argument::Conditional { rules, value } if rules_allow(Some(rules), env) => value
                .values()
                .into_iter()
                .map(|s| substitute(s, vars))
                .collect(),
            Argument::Conditional { .. } => Vec::new(),
        })
        .collect()
}

/// Split a player's JVM arguments on whitespace, keeping a quoted value that
/// contains a space as one argument (`-Dx="1.8.9 Forge"`,
/// `-javaagent:"C:\x\1.8.9 Forge\a.jar"`). Single and double quotes are
/// removed. A backslash is a Windows path separator, not an escape: a shell
/// splitter would turn `C:\Users` into `C:Users`. An unclosed quote is not
/// guessed at and falls back to splitting on whitespace.
pub(crate) fn split_jvm_args(args: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in args.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            None => {
                cur.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return args.split_whitespace().map(str::to_owned).collect();
    }
    if started {
        out.push(cur);
    }
    out
}

/// Replace every `${key}` with its value; unknown keys are left untouched.
pub fn substitute(template: &str, vars: &Placeholders) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find('}') {
            Some(end) => {
                let key = &after[..end];
                match vars.get(key) {
                    Some(v) => out.push_str(v),
                    None => {
                        log::debug!("unknown launch placeholder ${{{key}}}");
                        out.push_str(&rest[start..start + 2 + end + 1]);
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn vars() -> Placeholders {
        HashMap::from([
            ("auth_player_name", "Steve".to_string()),
            ("natives_directory", "N".to_string()),
            ("classpath", "CP".to_string()),
            ("resolution_width", "800".to_string()),
            ("resolution_height", "600".to_string()),
        ])
    }

    fn windows_env() -> RuleEnv {
        RuleEnv {
            os: "windows",
            arch: "x86_64",
            features: HashSet::from(["has_custom_resolution"]),
        }
    }

    fn opts() -> JvmOptions {
        JvmOptions {
            min_memory_mb: 512,
            max_memory_mb: 2048,
            resolution: (800, 600),
            ..JvmOptions::default()
        }
    }

    #[test]
    fn java4shell_flags_precede_custom_jvm_args() {
        let version: VersionJson =
            serde_json::from_str(r#"{"id":"1.21","mainClass":"M"}"#).unwrap();
        let user_args = split_jvm_args("-Xmx4G -XX:+UseG1GC");
        for major in [17, 21, 25, 27] {
            let mut options = opts();
            options.extra = java4shell_flags(Some(major), true, &user_args)
                .iter()
                .map(|&arg| arg.to_owned())
                .chain(user_args.iter().cloned())
                .collect();
            let args = build(&version, &windows_env(), &vars(), &options);
            assert_eq!(
                &args[..5],
                [
                    "-Xms512M",
                    "-XX:+UnlockDiagnosticVMOptions",
                    "-XX:-UseBASE64Intrinsics",
                    "-Xmx4G",
                    "-XX:+UseG1GC",
                ]
            );
        }
    }

    #[test]
    fn java4shell_flags_skip_older_or_unknown_java_and_non_native_x86_64() {
        for major in [Some(8), Some(11), None] {
            assert!(java4shell_flags(major, true, &[]).is_empty());
        }
        // Both native ARM64 and Rosetta pass false.
        assert!(java4shell_flags(Some(21), false, &[]).is_empty());
    }

    #[test]
    fn java4shell_flags_respect_explicit_intrinsic_settings() {
        for arg in ["-XX:+UseBASE64Intrinsics", "-XX:-UseBASE64Intrinsics"] {
            assert!(java4shell_flags(Some(21), true, &[arg.into()]).is_empty());
        }
    }

    #[test]
    fn substitution() {
        let v = vars();
        assert_eq!(
            substitute("--name=${auth_player_name}!", &v),
            "--name=Steve!"
        );
        assert_eq!(substitute("${unknown}/${classpath}", &v), "${unknown}/CP");
        assert_eq!(substitute("tail ${open", &v), "tail ${open");
    }

    #[test]
    fn modern_arguments_respect_rules() {
        let version: VersionJson = serde_json::from_str(
            r#"{"id":"1.21","mainClass":"M","arguments":{
                "game":["--username","${auth_player_name}",
                    {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}"]},
                    {"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"}],
                "jvm":[{"rules":[{"action":"allow","os":{"name":"osx"}}],"value":"-XstartOnFirstThread"},
                    "-Djava.library.path=${natives_directory}","-cp","${classpath}"]}}"#,
        )
        .unwrap();
        let args = build(&version, &windows_env(), &vars(), &opts());
        assert_eq!(
            args,
            [
                "-Xms512M",
                "-Xmx2048M",
                "-Djava.library.path=N",
                "-cp",
                "CP",
                "M",
                "--username",
                "Steve",
                "--width",
                "800"
            ]
        );
    }

    #[test]
    fn legacy_arguments_get_resolution_and_fullscreen() {
        let version: VersionJson = serde_json::from_str(
            r#"{"id":"1.8.9","mainClass":"M","minecraftArguments":"--username ${auth_player_name}"}"#,
        )
        .unwrap();
        let windowed = build(&version, &windows_env(), &vars(), &opts());
        assert_eq!(&windowed[2..5], ["-Djava.library.path=N", "-cp", "CP"]);
        assert!(windowed.ends_with(&[
            "--width".into(),
            "800".into(),
            "--height".into(),
            "600".into()
        ]));

        let full = build(
            &version,
            &windows_env(),
            &vars(),
            &JvmOptions {
                fullscreen: true,
                logging_arg: Some("-Dlog".into()),
                ..opts()
            },
        );
        assert!(full.contains(&"-Dlog".to_string()));
        assert_eq!(full.last().unwrap(), "--fullscreen");
        assert!(!full.contains(&"--width".to_string()));
    }

    #[test]
    fn jvm_arguments_keep_quoted_spaces_and_windows_paths() {
        assert_eq!(
            split_jvm_args(r#"-Dx="1.8.9 Forge" -Dbar='a b' -Xmx2G"#),
            ["-Dx=1.8.9 Forge", "-Dbar=a b", "-Xmx2G"]
        );
        assert_eq!(
            split_jvm_args(
                r#"-javaagent:"C:\x\1.8.9 Forge\a.jar" -Djava.library.path=C:\Users\me\natives"#
            ),
            [
                r"-javaagent:C:\x\1.8.9 Forge\a.jar",
                r"-Djava.library.path=C:\Users\me\natives",
            ]
        );
        // An unclosed quote is not guessed at: split on whitespace, as before.
        assert_eq!(
            split_jvm_args(r#"-Dfoo="1.8.9 Forge"#),
            [r#"-Dfoo="1.8.9"#, "Forge"]
        );
    }

    #[test]
    fn launcher_minimum_cannot_exceed_a_custom_maximum() {
        let version: VersionJson =
            serde_json::from_str(r#"{"id":"1.20.1","mainClass":"M"}"#).unwrap();
        let args = build(
            &version,
            &windows_env(),
            &vars(),
            &JvmOptions {
                extra: vec!["-Xmx256M".into()],
                ..opts()
            },
        );
        assert_eq!(
            &args[..3],
            ["-Xms256M", "-Xmx256M", "-Djava.library.path=N"]
        );
    }

    #[test]
    fn a_quoted_forge_token_does_not_become_the_main_class() {
        let version: VersionJson = serde_json::from_str(
            r#"{"id":"1.8.9","mainClass":"net.minecraft.launchwrapper.Launch","minecraftArguments":"--tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker"}"#,
        )
        .unwrap();
        let mut options = opts();
        options.extra = split_jvm_args(r#"-Dx="1.8.9 Forge""#);
        let args = build(&version, &windows_env(), &vars(), &options);
        let cp = args.iter().position(|a| a == "-cp").unwrap();
        // Classpath value, then the main class. Nothing may land in front of it.
        assert_eq!(args[cp + 2], "net.minecraft.launchwrapper.Launch");
        assert!(args.iter().any(|a| a == "-Dx=1.8.9 Forge"));
        assert!(!args.iter().any(|a| a == "Forge"));
    }
}
