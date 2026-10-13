//! Plain-language explanations for common Minecraft crashes, read from the
//! game log and crash report: missing or wrong-version mods, broken mixins,
//! the wrong Java, running out of memory, graphics drivers.

use std::sync::OnceLock;

use regex::Regex;

/// What went wrong, for people rather than stack traces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    pub title: String,
    pub detail: String,
    /// Mods involved (names or ids), if the logs say.
    pub mods: Vec<String>,
    /// A game or loader file is damaged: the next start checks every library and fetches it again.
    pub repair: bool,
}

impl Diagnosis {
    /// "This rule matched but has nothing to say": keep looking.
    fn none() -> Self {
        Self::new("", "", Vec::new())
    }

    fn new(title: impl Into<String>, detail: impl Into<String>, mods: Vec<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            mods,
            repair: false,
        }
    }
}

/// A pack this big (mods in its folder)…
const BIG_PACK_MODS: usize = 80;
/// …with less memory than this hardly gets through the loader reading its mods.
const BIG_PACK_LEAST_MB: u32 = 3 * 1024;

/// `-Xmx1918M` (or `-Xmx2G`) in how Java was started → MB.
pub fn max_memory_mb(java: &str) -> Option<u32> {
    let flag = java
        .split_whitespace()
        .find_map(|w| w.strip_prefix("-Xmx"))?;
    let (number, unit) = flag.split_at(flag.find(|c: char| !c.is_ascii_digit())?);
    let n: u32 = number.parse().ok()?;
    match unit.to_ascii_lowercase().as_str() {
        "m" => Some(n),
        "g" => n.checked_mul(1024),
        _ => None,
    }
}

/// A big pack that got too little memory (the computer has little to give): what to tell the
/// player before it starts, or `None` when the memory is enough.
pub fn memory_warning(given_mb: u32, mods: usize, total_mb: Option<u64>) -> Option<String> {
    if mods < BIG_PACK_MODS || given_mb >= BIG_PACK_LEAST_MB {
        return None;
    }
    let gb = |mb: u64| format!("{:.1} GB", mb as f64 / 1024.0);
    let computer = total_mb.map_or_else(String::new, |t| format!(" (this computer has {})", gb(t)));
    Some(format!(
        "Minecraft can get only {}{computer}, and {mods} mods usually need 5 GB or more: it may not start. Fewer mods, or a computer with more memory, will.",
        gb(u64::from(given_mb))
    ))
}

/// A start that failed while the loader read a big pack with too little memory: classes of its
/// own libraries "not found" are memory running out, not a damaged file.
pub fn too_little_memory(given_mb: u32, mods: usize, total_mb: Option<u64>) -> Option<Diagnosis> {
    memory_warning(given_mb, mods, total_mb)
        .map(|detail| Diagnosis::new("Not enough memory for this many mods", detail, Vec::new()))
}

/// Most crashes print one of these; each is a regex and what it means.
struct Rule {
    pattern: &'static str,
    explain: fn(&regex::Captures, &str) -> Diagnosis,
}

/// Checked in order: the most specific causes first.
const RULES: &[Rule] = &[
    Rule {
        // Fabric: "- Mod 'Sodium Extra' (sodium-extra) 0.5 requires any version of fabric-api, which is missing!"
        pattern: r"Mod '([^']+)' \(([^)]+)\) \S+ requires (?:any version|version [^,]+?) of ([\w\-.]+), which is missing",
        explain: |c, text| {
            let missing = collect(text, 3);
            let needs = missing.join(", ");
            Diagnosis::new(
                format!("A mod needs {needs}"),
                format!(
                    "{} needs {needs}, which isn't installed. Add it from Modrinth in the instance's Mods page.",
                    &c[1]
                ),
                missing,
            )
        },
    },
    Rule {
        // Fabric: "... requires version 1.21.4 of minecraft, but only the wrong version is present: 1.21.6!"
        pattern: r"Mod '([^']+)' \(([^)]+)\) \S+ requires (?:version )?([^,]+?) of (minecraft|java|fabricloader), but only the wrong version is present: ([^!\s]+)",
        explain: |c, _| {
            let what = match &c[4] {
                "minecraft" => "Minecraft",
                "java" => "Java",
                _ => "Fabric Loader",
            };
            Diagnosis::new(
                format!("{} is for a different {what} version", &c[1]),
                format!(
                    "{} needs {what} {}, but this instance has {}. Update the mod or remove it.",
                    &c[1], &c[3], &c[5]
                ),
                vec![c[1].to_owned()],
            )
        },
    },
    Rule {
        // Fabric: "- Mod 'X' (x) 1.0 is incompatible with any version of mod 'Y' (y), but a matching version is present"
        pattern: r"Mod '([^']+)' \(([^)]+)\) \S+ is incompatible with [^']*'([^']+)'",
        explain: |c, _| {
            Diagnosis::new(
                format!("{} and {} don't work together", &c[1], &c[3]),
                format!(
                    "{} doesn't work alongside {}. Remove one of them.",
                    &c[1], &c[3]
                ),
                vec![c[1].to_owned(), c[3].to_owned()],
            )
        },
    },
    Rule {
        // NeoForge/Forge: "Mod ID: 'jei', Requested by: 'x', Expected range: '[...]', Actual version: '[MISSING]'"
        pattern: r"Mod ID: '([^']+)', Requested by: '([^']+)', Expected range: '([^']*)', Actual version: '([^']*)'",
        explain: |c, _| {
            let missing = &c[4] == "[MISSING]";
            Diagnosis::new(
                if missing {
                    format!("A mod needs {}", &c[1])
                } else {
                    format!("{} is the wrong version", &c[1])
                },
                if missing {
                    format!("{} needs {}, which isn't installed.", &c[2], &c[1])
                } else {
                    format!(
                        "{} needs {} {}, but {} is installed.",
                        &c[2], &c[1], &c[3], &c[4]
                    )
                },
                vec![c[1].to_owned(), c[2].to_owned()],
            )
        },
    },
    Rule {
        pattern: r"(?i)found (?:\d+ )?duplicate(?:d)? mods?|Duplicate mods? found|is provided by (?:both|multiple)",
        explain: |_, _| {
            Diagnosis::new(
                "The same mod is installed twice",
                "Two files provide the same mod (often an old and a new version). Keep only the newest one.",
                Vec::new(),
            )
        },
    },
    Rule {
        // Fabric: "Mixin apply for mod sodium failed sodium.mixins.json:..."
        pattern: r"Mixin apply for mod ([\w\-]+) failed",
        explain: |c, _| broken_mod(&c[1]),
    },
    Rule {
        pattern: r"Mixin \[[^\]]+\] from mod ([\w\-]+)",
        explain: |c, _| broken_mod(&c[1]),
    },
    Rule {
        pattern: r"class file version (\d+)\.\d+\).*?class file versions up to (\d+)\.\d+",
        explain: |c, _| {
            let java = |v: &str| v.parse::<u32>().map_or(0, |v| v.saturating_sub(44));
            Diagnosis::new(
                format!("This needs Java {}", java(&c[1])),
                format!(
                    "Something here was built for Java {}, but the game runs on Java {}. Clear the instance's custom Java so Arctic picks the right one.",
                    java(&c[1]),
                    java(&c[2])
                ),
                Vec::new(),
            )
        },
    },
    Rule {
        // Not the heap: a thread ran out of its own stack (endless recursion in a mod, or a driver).
        pattern: r"OutOfMemoryError: Out of stack space|java\.lang\.StackOverflowError",
        explain: |_, text| {
            let vulkan = text.contains("net.vulkanmod") || text.contains("vulkanmod");
            Diagnosis::new(
                "The game ran out of stack space",
                if vulkan {
                    "A part of the game went in circles until its thread's stack was full. That isn't RAM: more memory won't help. VulkanMod is in this game; update it and the graphics driver, or play without it."
                } else {
                    "A part of the game went in circles until its thread's stack was full. That isn't RAM: more memory won't help. A mod is usually behind it; try updating your mods."
                },
                Vec::new(),
            )
        },
    },
    Rule {
        // The system couldn't give Java the memory it was told to take (Windows: the paging file is
        // too small for it; "errno=1455").
        pattern: r"There is insufficient memory for the Java Runtime Environment|Native memory allocation \(mmap\) failed|errno=1455",
        explain: |_, _| {
            Diagnosis::new(
                "Not enough memory for the memory setting",
                "Minecraft was told to take more memory than this computer could give it. Lower the Maximum in Settings → Memory (or the instance's own setting), or close other programs.",
                Vec::new(),
            )
        },
    },
    Rule {
        // VulkanMod (or the Vulkan driver under it) failed: a lost device, no swap chain, its own bugs.
        pattern: r"VK_ERROR_[A-Z_]+|at net\.vulkanmod\.|No GLCapabilities instance set for the current thread",
        explain: |c, text| {
            if !(c[0].starts_with("VK_ERROR") || text.contains("vulkanmod")) {
                return Diagnosis::none();
            }
            // A window it can't draw to: usually the wrong one of two graphics cards.
            let card = Regex::new(r"Backend API: Vulkan [\d.]+, (\w+)")
                .ok()
                .and_then(|re| re.captures(text).map(|v| v[1].to_owned()));
            let hint = match card {
                Some(card) if text.contains("Failed to create swap chain") => format!(
                    " VulkanMod used the {card} graphics card, which can't show this window: on a computer with two cards, pick the other one in VulkanMod's video settings (Device)."
                ),
                _ => String::new(),
            };
            let code = if c[0].starts_with("VK_ERROR") {
                format!(" ({})", &c[0])
            } else {
                String::new()
            };
            Diagnosis::new(
                "VulkanMod crashed",
                format!(
                    "VulkanMod or the graphics driver under it failed{code}.{hint} Update your graphics driver and VulkanMod, or remove VulkanMod to play with the game's own renderer."
                ),
                vec!["vulkanmod".into()],
            )
        },
    },
    Rule {
        pattern: r"java\.lang\.OutOfMemoryError",
        explain: |_, _| {
            Diagnosis::new(
                "Minecraft ran out of memory",
                "Give it more memory in Settings → Memory (or the instance's own setting), or use fewer or lighter mods and resource packs.",
                Vec::new(),
            )
        },
    },
    Rule {
        pattern: r"Could not reserve enough space for|Invalid maximum heap size",
        explain: |_, _| {
            Diagnosis::new(
                "Too much memory for this PC",
                "Java couldn't set aside the memory Minecraft asked for. Lower it in Settings → Memory.",
                Vec::new(),
            )
        },
    },
    Rule {
        // The crashing frame of a native crash ("# C  [nvoglv64.dll+0x1234]"), not
        // any mention: Sodium logs the driver's file at every start.
        pattern: r"(?im)^#\s*C\s+\[(atio6axx|atioglxx|atig6pxx|amdxc64|nvoglv64|nvoglv32|ig\w*icd(?:32|64))\.dll",
        explain: |c, _| {
            let vendor = match c[1].to_ascii_lowercase().as_str() {
                d if d.starts_with("nv") => "NVIDIA",
                d if d.starts_with("ig") => "Intel",
                _ => "AMD",
            };
            Diagnosis::new(
                "The graphics driver crashed",
                format!(
                    "The crash happened inside the {vendor} graphics driver. Update it from {vendor}'s website; shader packs or overlays can also trigger this."
                ),
                Vec::new(),
            )
        },
    },
    Rule {
        pattern: r"(?i)Pixel format not accelerated|driver does not appear to support OpenGL|GLFW error 6554[23]|Failed to create (?:the )?window",
        explain: |_, _| {
            Diagnosis::new(
                "OpenGL isn't available",
                "Minecraft couldn't start its graphics. Install your graphics card's driver (Windows' basic display driver doesn't support OpenGL).",
                Vec::new(),
            )
        },
    },
    Rule {
        // Fabric/Forge crash reports name suspects.
        pattern: r"Suspected Mods?:\s*([^\n]+)",
        explain: |c, _| {
            let mods: Vec<String> = c[1]
                .split(',')
                .map(|m| m.trim().to_owned())
                .filter(|m| {
                    !m.is_empty() && !m.eq_ignore_ascii_case("none") && !m.contains("Unknown")
                })
                .collect();
            if mods.is_empty() {
                return Diagnosis::none();
            }
            Diagnosis::new(
                format!("Probably caused by {}", mods.join(", ")),
                "The crash report points at these mods. Try updating them, or remove them to check.",
                mods,
            )
        },
    },
    Rule {
        // A class of the game's or the loader's own libraries can't be found: the file is damaged
        // (a jar the right size whose insides can't be read), not a mod made for another version.
        // "NoClassDefFoundError: org/spongepowered/asm/mixin/throwables/MixinError" from Fabric's
        // Mixin bootstrap, before any mod runs.
        pattern: r"(?:NoClassDefFoundError|ClassNotFoundException):? ((?:org[./]spongepowered[./]asm|org[./]objectweb[./]asm|net[./]fabricmc[./]loader|org[./]quiltmc[./]loader|cpw[./]mods|net[./]minecraftforge[./]fml|net[./]neoforged[./]fml|org[./]lwjgl|com[./]mojang[./](?:blaze3d|authlib|brigadier|datafixers|logging|serialization))[\w./$]*)",
        explain: |c, _| Diagnosis {
            repair: true,
            ..Diagnosis::new(
                "A game file is damaged",
                format!(
                    "Part of Minecraft or its mod loader couldn't be read ({}). The next time you press Play, Arctic checks the game's files (fetching fresh copies of damaged ones) and starts this instance without its start-up speed-ups. If it happens again, please send the report.",
                    c[1].replace('/', ".")
                ),
                Vec::new(),
            )
        },
    },
    Rule {
        pattern: r"java\.lang\.(NoSuchMethodError|NoSuchFieldError|NoClassDefFoundError|AbstractMethodError)",
        explain: |_, _| {
            Diagnosis::new(
                "A mod is for a different version",
                "A mod is calling code that isn't there: it's probably made for another Minecraft version or needs a different version of another mod. Update your mods.",
                Vec::new(),
            )
        },
    },
    Rule {
        pattern: r"EXCEPTION_ACCESS_VIOLATION",
        explain: |_, _| {
            Diagnosis::new(
                "Minecraft crashed in native code",
                "Usually a graphics driver, an overlay (recording or FPS tools) or antivirus. Update your graphics driver and try without overlays.",
                Vec::new(),
            )
        },
    },
];

fn broken_mod(id: &str) -> Diagnosis {
    Diagnosis::new(
        format!("{id} failed to load"),
        format!(
            "{id} couldn't hook into the game: it's likely made for another Minecraft version or clashes with another mod. Update or remove it."
        ),
        vec![id.to_owned()],
    )
}

/// Every match of capture group `group` of the first rule (deduplicated).
fn collect(text: &str, group: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in compiled()[0].captures_iter(text) {
        let name = c[group].to_owned();
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

fn compiled() -> &'static [Regex] {
    static COMPILED: OnceLock<Vec<Regex>> = OnceLock::new();
    COMPILED.get_or_init(|| {
        RULES
            .iter()
            .map(|r| Regex::new(r.pattern).expect("crash patterns are valid"))
            .collect()
    })
}

/// The largest amount of text looked at (the end of it, where crashes are).
const MAX_TEXT: usize = 2 * 1024 * 1024;

/// Explain a crash from the game log and/or crash report text.
pub fn diagnose(text: &str) -> Option<Diagnosis> {
    let start = text.len().saturating_sub(MAX_TEXT);
    let start = (start..text.len())
        .find(|&i| text.is_char_boundary(i))
        .unwrap_or(0);
    let text = &text[start..];
    RULES
        .iter()
        .zip(compiled())
        .filter_map(|(rule, re)| re.captures(text).map(|c| (rule.explain)(&c, text)))
        .find(|d| !d.title.is_empty())
}

/// A crash as it's sent: the log's end and the crash report, with anything
/// personal taken out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub launcher: String,
    pub game: String,
    pub loader: String,
    pub os: String,
    pub title: String,
    pub log: String,
}

/// Lines of the game log's end that are sent.
const REPORT_LOG_LINES: usize = 150;
/// Most of the log's end, and of the crash report's start, that are sent (bytes).
const REPORT_LOG_BYTES: usize = 16 * 1024;
const REPORT_CRASH_BYTES: usize = 24 * 1024;
/// How Java was started: its flags and the classpath's file names.
const REPORT_JAVA_BYTES: usize = 6 * 1024;

impl Report {
    /// `lines` is the game's log, `crash_report` the file Minecraft wrote (if it did), `java`
    /// how Java was started (see `LaunchPlan::java_summary`). `title` is what Arctic made of it,
    /// or empty.
    pub fn build(
        game: &str,
        loader: &str,
        title: &str,
        lines: &[&str],
        crash_report: Option<&str>,
        java: Option<&str>,
    ) -> Self {
        let skip = lines.len().saturating_sub(REPORT_LOG_LINES);
        let tail = lines[skip..].join("\n");
        let mut text = tail_of(&tail, REPORT_LOG_BYTES).to_owned();
        if let Some(java) = java {
            text.push_str("\n\n---- java ----\n");
            text.push_str(head_of(java, REPORT_JAVA_BYTES));
        }
        if let Some(report) = crash_report {
            text.push_str("\n\n---- crash report ----\n");
            text.push_str(head_of(report, REPORT_CRASH_BYTES));
        }
        Self {
            launcher: env!("CARGO_PKG_VERSION").to_owned(),
            game: game.to_owned(),
            loader: loader.to_owned(),
            os: std::env::consts::OS.to_owned(),
            title: title.to_owned(),
            log: scrub(&text, &Own::this_pc()),
        }
    }
}

fn head_of(text: &str, max: usize) -> &str {
    let mut end = max.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

fn tail_of(text: &str, max: usize) -> &str {
    let mut start = text.len().saturating_sub(max);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// The end of `text` (at most `max` bytes), with names and tokens taken out.
pub fn scrubbed_tail(text: &str, max: usize) -> String {
    scrub(tail_of(text, max), &Own::this_pc())
}

/// What identifies this PC's owner in paths and logs.
pub struct Own {
    pub home: Option<String>,
    pub user: Option<String>,
}

impl Own {
    fn this_pc() -> Self {
        Self {
            home: std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .ok(),
            user: std::env::var("USERNAME")
                .or_else(|_| std::env::var("USER"))
                .ok(),
        }
    }
}

/// Take out what a stranger shouldn't read: the home folder and user name,
/// sign-in tokens, e-mail addresses and IP addresses.
pub fn scrub(text: &str, own: &Own) -> String {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            (
                r"(?i)(--(?:accessToken|uuid|username|xuid|clientId|session)\s+)\S+",
                "${1}<hidden>",
            ),
            (
                r"(?i)((?:access_?token|session_?id|bearer|token)[\x22'\s:=]+)[A-Za-z0-9._\-]{20,}",
                "${1}<hidden>",
            ),
            (
                r"eyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}",
                "<hidden>",
            ),
            (r"Setting user: \S+", "Setting user: <player>"),
            (
                r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}",
                "<email>",
            ),
            (r"\b(?:\d{1,3}\.){3}\d{1,3}\b", "<ip>"),
        ]
        .into_iter()
        .filter_map(|(p, r)| Regex::new(p).ok().map(|re| (re, r)))
        .collect()
    });
    let mut out = text.to_owned();
    if let Some(home) = own.home.as_deref().filter(|h| h.len() > 3) {
        for variant in [home.to_owned(), home.replace('\\', "/")] {
            if let Ok(re) = Regex::new(&format!("(?i){}", regex::escape(&variant))) {
                out = re.replace_all(&out, "<home>").into_owned();
            }
        }
    }
    if let Some(user) = own.user.as_deref().filter(|u| u.len() >= 3)
        && let Ok(re) = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(user)))
    {
        out = re.replace_all(&out, "<user>").into_owned();
    }
    for (re, replacement) in patterns {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_big_pack_with_too_little_memory_is_told_so() {
        assert_eq!(
            max_memory_mb("Java x, flags: -Xms512M -Xmx1918M -XX:+UseG1GC"),
            Some(1918)
        );
        assert_eq!(max_memory_mb("flags: -Xmx4G"), Some(4096));
        assert_eq!(max_memory_mb("flags: -Xms512M"), None);
        let d = too_little_memory(1918, 213, Some(3966)).unwrap();
        assert_eq!(d.title, "Not enough memory for this many mods");
        assert!(
            d.detail.contains("1.9 GB")
                && d.detail.contains("3.9 GB")
                && d.detail.contains("213 mods")
        );
        assert!(!d.repair);
        // Enough memory, or a small pack: nothing to say.
        assert!(too_little_memory(6144, 213, Some(16384)).is_none());
        assert!(too_little_memory(1918, 20, Some(3966)).is_none());
    }

    fn title(text: &str) -> String {
        diagnose(text).map(|d| d.title).unwrap_or_default()
    }

    #[test]
    fn the_crash_reports_get_the_right_explanation() {
        // #01: Java couldn't get the 32 GB it was told to take.
        assert_eq!(
            title("OpenJDK 64-Bit Server VM warning: INFO: os::commit_memory(0x0, 34359738368, 0) failed; error='...' (DOS error/errno=1455)
# There is insufficient memory for the Java Runtime Environment to continue.
# Native memory allocation (mmap) failed to map 34359738368 bytes. Error detail: G1 virtual space"),
            "Not enough memory for the memory setting"
        );
        // #17-21: a thread's stack, not the heap.
        assert_eq!(
            title("java.lang.OutOfMemoryError: Out of stack space."),
            "The game ran out of stack space"
        );
        // #03, #09-16, #04: VulkanMod and its driver.
        assert_eq!(
            title(
                "java.lang.RuntimeException: Failed to create swap chain: VK_ERROR_INITIALIZATION_FAILED"
            ),
            "VulkanMod crashed"
        );
        let d = diagnose("java.lang.RuntimeException: Failed to create swap chain: VK_ERROR_INITIALIZATION_FAILED\n\tBackend API: Vulkan 1.4.354, AMD").unwrap();
        assert!(
            d.detail.contains("used the AMD graphics card"),
            "{}",
            d.detail
        );
        assert_eq!(
            title(
                "java.lang.NullPointerException: Cannot invoke \"x\"
	at net.vulkanmod.render.chunk.WorldRenderer.allChanged(WorldRenderer.java:1)"
            ),
            "VulkanMod crashed"
        );
        // The heap really full is still about memory.
        assert_eq!(
            title("java.lang.OutOfMemoryError: Java heap space"),
            "Minecraft ran out of memory"
        );
    }

    #[test]
    fn a_missing_loader_class_is_a_damaged_file_not_a_mod() {
        // Crash report #22: Fabric's Mixin bootstrap, before any mod ran.
        let log = "[main/ERROR] Uncaught exception in thread \"main\"
    java.lang.NoClassDefFoundError: org/spongepowered/asm/mixin/throwables/MixinError
    	at net.fabricmc.loader.impl.launch.FabricMixinBootstrap.init(FabricMixinBootstrap.java:59)
    Caused by: java.lang.ClassNotFoundException: org.spongepowered.asm.mixin.throwables.MixinError";
        let d = diagnose(log).unwrap();
        assert_eq!(d.title, "A game file is damaged");
        assert!(d.repair);
        assert!(
            d.detail
                .contains("org.spongepowered.asm.mixin.throwables.MixinError"),
            "{}",
            d.detail
        );
        // A mod's own class missing is still about the mod.
        let d = diagnose("java.lang.NoClassDefFoundError: com/example/coolmod/Thing").unwrap();
        assert_eq!(d.title, "A mod is for a different version");
        assert!(!d.repair);
    }

    #[test]
    fn missing_fabric_dependencies_are_named() {
        let log = "Incompatible mods found!\n\
            - Mod 'Sodium Extra' (sodium-extra) 0.5.4 requires any version of fabric-api, which is missing!\n\
            - Mod 'Mod Menu' (modmenu) 9.0 requires version 0.90.0 or later of fabric-api, which is missing!\n\
            - Mod 'Other' (other) 1.0 requires any version of cloth-config, which is missing!";
        let d = diagnose(log).unwrap();
        assert_eq!(d.mods, ["fabric-api", "cloth-config"]);
        assert!(d.detail.contains("Sodium Extra"));
    }

    #[test]
    fn wrong_minecraft_version() {
        let log = "- Mod 'Old Mod' (old) 1.0 requires version 1.20.1 of minecraft, but only the wrong version is present: 1.21.6!";
        let d = diagnose(log).unwrap();
        assert_eq!(d.title, "Old Mod is for a different Minecraft version");
        assert!(d.detail.contains("1.20.1") && d.detail.contains("1.21.6"));
    }

    #[test]
    fn java_version() {
        let log = "java.lang.UnsupportedClassVersionError: x has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        assert_eq!(title(log), "This needs Java 21");
    }

    #[test]
    fn other_causes() {
        assert_eq!(
            title("java.lang.OutOfMemoryError: Java heap space"),
            "Minecraft ran out of memory"
        );
        assert_eq!(
            title("# Problematic frame:\n# C  [atio6axx.dll+0x1234]"),
            "The graphics driver crashed"
        );
        assert_eq!(
            title("Mixin apply for mod sodium failed sodium.mixins.json:X"),
            "sodium failed to load"
        );
        assert_eq!(
            title("Suspected Mods: Iris (iris), Sodium (sodium)"),
            "Probably caused by Iris (iris), Sodium (sodium)"
        );
        assert!(diagnose("Suspected Mods: None\nnothing else").is_none());
        assert!(diagnose("[Render thread/INFO] Stopping!").is_none());
        // Sodium names the driver's file at every start; that alone isn't a crash.
        assert!(
            diagnose(r"[main/INFO]: Found graphics adapter: AdapterInfo{vendor=NVIDIA, openglIcdFilePath='C:\Windows\System32\nvoglv64.dll'}")
                .is_none()
        );
    }

    #[test]
    fn huge_logs_are_cut_on_a_character_boundary() {
        let mut log = "é".repeat(MAX_TEXT);
        log.push_str("java.lang.OutOfMemoryError");
        assert_eq!(title(&log), "Minecraft ran out of memory");
    }

    #[test]
    fn reports_lose_what_is_personal() {
        let own = Own {
            home: Some(r"C:\Users\Burak".into()),
            user: Some("Burak".into()),
        };
        let text = "Setting user: Steve\nat C:\\Users\\Burak\\AppData\\x.jar\n\
                    --accessToken eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ4In0.abcdefghijk1 --uuid 1234\n\
                    mail me at a.b@example.com from 192.168.1.20, Burak";
        let clean = scrub(text, &own);
        for secret in [
            "Steve",
            "Burak",
            "eyJhbG",
            "a.b@example.com",
            "192.168",
            "1234",
        ] {
            assert!(!clean.contains(secret), "{secret} left in: {clean}");
        }
        assert!(clean.contains("<home>") && clean.contains("<hidden>") && clean.contains("<ip>"));
    }

    #[test]
    fn a_report_fits_what_the_server_keeps() {
        let line = "x".repeat(400);
        let lines: Vec<&str> = (0..500).map(|_| line.as_str()).collect();
        let report = Report::build(
            "1.21.4",
            "fabric",
            "Crash",
            &lines,
            Some(&"y".repeat(200_000)),
            Some(&"z".repeat(50_000)),
        );
        assert!(report.log.len() < 48 * 1024);
        assert!(report.log.contains("---- crash report ----"));
        assert!(report.log.contains("---- java ----"));
    }
}
