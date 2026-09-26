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
}
