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
    let mut args = vec![
        format!("-Xms{}M", opts.min_memory_mb),
        format!("-Xmx{}M", opts.max_memory_mb),
    ];
    args.extend(opts.extra.iter().cloned());

    match &version.arguments {
        Some(a) if !a.jvm.is_empty() => args.extend(expand(&a.jvm, env, vars)),
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
