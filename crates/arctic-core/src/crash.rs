//! Plain-language explanations for common Minecraft crashes, read from the
//! game log and crash report: missing or wrong-version mods, broken mixins,
//! the wrong Java, running out of memory, graphics drivers.

use std::{borrow::Cow, path::Path, sync::OnceLock};

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::launch::args::{CUSTOM_MAX_HEAP_SOURCE, heap_size_mb};
use crate::launch::exit_diagnostics::ExitDiagnostics;

/// A fix the launcher can do for the player in one click (see `crash_fix`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    /// Switch these mods off (names as the log gives them; the files are renamed to
    /// `.jar.disabled`, so the Mods page can switch them back on).
    DisableMods(Vec<String>),
    /// Move the instance's Fabric Loader to the newest stable build.
    UpdateLoader,
}

/// What went wrong, for people rather than stack traces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    pub title: String,
    pub detail: String,
    /// Mods involved (names or ids), if the logs say.
    pub mods: Vec<String>,
    /// A game or loader library could not load: check its files on the next start.
    pub repair: bool,
    /// What the launcher can do about it, if anything.
    pub fix: Option<Fix>,
}

pub const IRIS_SHADER_LOADING_TITLE: &str =
    "The game was closed while Iris was loading a shader pack";
pub const IRIS_SHADER_LOADING_DETAIL: &str = "Shader packs can take a while to load, especially on slower graphics cards. Give Iris time to finish or try a lighter shader pack.";

impl Diagnosis {
    /// "This rule matched but has nothing to say": keep looking.
    fn none() -> Self {
        Self::new("", "", Vec::new())
    }

    pub(crate) fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        mods: Vec<String>,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            mods,
            repair: false,
            fix: None,
        }
    }
}

/// A pack this big (mods in its folder)…
const BIG_PACK_MODS: usize = 80;
/// …with less memory than this hardly gets through the loader reading its mods.
const BIG_PACK_LEAST_MB: u32 = 3 * 1024;
const CUSTOM_JAVA_FIX: &str =
    "If you set custom Java, turn it off for this instance or in Settings to use Arctic's runtime.";
const JAVA_8_MISMATCH_TITLE: &str = "This version needs Java 8, but is running on Java";

/// The last `-Xmx` in how Java was started, in MiB.
pub fn max_memory_mb(java: &str) -> Option<u32> {
    let flags = java.lines().rev().find_map(java_summary_parts);
    let flag = flags
        .map_or(java, |(_, flags)| flags)
        .split_whitespace()
        .rev()
        .find_map(|w| w.strip_prefix("-Xmx"))?;
    heap_size_mb(flag)
}

fn java_summary_parts(line: &str) -> Option<(&str, &str)> {
    let (java, flags) = line.strip_prefix("Java ")?.split_once(", flags: ")?;
    let flags = flags.split_once(" | classpath:").map_or(flags, |(f, _)| f);
    Some((java, flags))
}

fn custom_max_memory_mb(text: &str) -> Option<u32> {
    let (java, flags) = text.lines().rev().find_map(java_summary_parts)?;
    let mut sizes = flags
        .split_whitespace()
        .rev()
        .filter_map(|w| w.strip_prefix("-Xmx"));
    let effective = sizes.next()?;
    // Older Arctic summaries start with the launcher's heap flag and append the custom one.
    if java.ends_with(CUSTOM_MAX_HEAP_SOURCE) || sizes.next().is_some() {
        return heap_size_mb(effective);
    }
    None
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
        "Minecraft is starting with {}{computer} for {mods} mod files. A large modpack may need more memory. If it runs out, try fewer mods or close other programs.",
        gb(u64::from(given_mb))
    ))
}

/// Most crashes print one of these; each is a regex and what it means.
struct Rule {
    pattern: &'static str,
    explain: fn(&regex::Captures, &str) -> Diagnosis,
}

const OUT_OF_MEMORY: &str = r"java\.lang\.OutOfMemoryError";
const LINKAGE_ERROR: &str = r"java\.lang\.(NoSuchMethodError|NoSuchFieldError|NoClassDefFoundError|ClassNotFoundException|AbstractMethodError)";

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
        // JVM entry points: this fails before the loader can report an uncaught exception.
        pattern: r"(?:Could not find or load main class|ClassNotFoundException:) (net\.fabricmc\.loader\.(?:impl\.)?launch\.knot\.KnotClient|org\.quiltmc\.loader\.impl\.launch\.knot\.KnotClient|cpw\.mods\.bootstraplauncher\.BootstrapLauncher|cpw\.mods\.modlauncher\.Launcher|net\.minecraft\.launchwrapper\.Launch)(?:\s|$)",
        explain: |c, _| {
            let loader = if c[1].starts_with("net.fabricmc.") {
                "Fabric"
            } else if c[1].starts_with("org.quiltmc.") {
                "Quilt"
            } else if c[1].starts_with("net.minecraft.launchwrapper.") {
                "Forge"
            } else {
                "Forge/NeoForge"
            };
            Diagnosis {
                repair: true,
                ..Diagnosis::new(
                    format!("A game file was damaged (the {loader} loader)"),
                    format!(
                        "Java couldn't load the {loader} loader's main class ({}). Arctic will repair it on the next Play.",
                        &c[1]
                    ),
                    Vec::new(),
                )
            }
        },
    },
    Rule {
        pattern: r"(?is)SDL3\.dll.{0,1500}(?:error code\s*=\s*1114|DLL init.*failed)",
        explain: |_, _| {
            Diagnosis::new(
                "SDL could not initialize the graphics driver",
                "Windows returned ERROR_DLL_INIT_FAILED (1114) while loading SDL3.dll. Try turning off Settings → High-performance GPU, then restart the game. An old graphics driver can also cause this; update it from the graphics vendor.",
                Vec::new(),
            )
        },
    },
    Rule {
        pattern: r"OpenGL is not supported: Could not retrieve OpenGL functions",
        explain: |_, _| {
            Diagnosis::new(
                "OpenGL functions are unavailable",
                "The graphics backend could not load OpenGL functions. On Linux/XWayland, check that the GPU's OpenGL driver and XWayland GL support are installed and working; try another desktop session or update the graphics driver.",
                Vec::new(),
            )
        },
    },
    Rule {
        // Fatal Java module conflicts take precedence over Connector's earlier resolver hints.
        pattern: r"java\.lang\.module\.ResolutionException: Modules (\S+) and (\S+) export package (\S+) to module \S+",
        explain: |c, _| split_package(&c[1], &c[2], &c[3]),
    },
    Rule {
        pattern: r"java\.lang\.module\.ResolutionException: Module \S+ reads package (\S+) from both (\S+) and (\S+)",
        explain: |c, _| split_package(&c[2], &c[3], &c[1]),
    },
    Rule {
        // Fabric: "- Mod 'ModernFix' (modernfix) 5.2 requires version 0.16.10 or later of mod 'Fabric Loader' (fabricloader), but only the wrong version is present: 0.15.11!"
        pattern: r"Mod '([^']+)' \([^)]+\) \S+ requires (?:any version|version) (.+?) of (?:mod )?'[^']+' \(fabricloader\), but only the wrong version is present: ([^!\s]+)",
        explain: |c, text| {
            static LOADER: OnceLock<Regex> = OnceLock::new();
            let re = LOADER.get_or_init(|| Regex::new(
                r"Mod '([^']+)' \([^)]+\) \S+ requires [^\r\n]*? \(fabricloader\), but only the wrong version is present"
            ).unwrap());
            let mut names = Vec::new();
            for m in re.captures_iter(text) {
                if !names.iter().any(|name| name == &m[1]) {
                    names.push(m[1].to_owned());
                }
            }
            Diagnosis {
                fix: Some(Fix::UpdateLoader),
                ..Diagnosis::new(
                    "Fabric Loader is too old for these mods",
                    format!(
                        "{} need {} of Fabric Loader, but this instance has {}. Update Fabric Loader in the instance's settings (Loader), or remove those mods.",
                        names.join(", "),
                        &c[2],
                        &c[3]
                    ),
                    names,
                )
            }
        },
    },
    Rule {
        // Resolver conflicts must win over dependency and replacement hints, even in translated logs.
        pattern: r"NEG_HARD_DEP \S+ \S+ \{breaks \S+ @",
        explain: |_, text| {
            let re = Regex::new(r"NEG_HARD_DEP (\S+) \S+ \{breaks (\S+) @")
                .expect("resolver conflict pattern is valid");
            let names = Regex::new(r"(?:Mod|mod) '([^']+)' \(([^)]+)\)")
                .expect("mod display name pattern is valid");
            let name = |id: &str| {
                names
                    .captures_iter(text)
                    .find(|c| &c[2] == id)
                    .map_or_else(|| id.to_owned(), |c| c[1].to_owned())
            };
            resolver_causes(text)
                .find_map(|line| {
                    re.captures(line).map(|c| {
                        let first = name(&c[1]);
                        let second = name(&c[2]);
                        Diagnosis::new(
                            format!("{first} and {second} don't work together"),
                            format!("{first} doesn't work alongside {second}. Remove one of them."),
                            vec![first, second],
                        )
                    })
                })
                .unwrap_or_else(Diagnosis::none)
        },
    },
    Rule {
        // Fabric: "- Mod 'X' (x) 1.0 is incompatible with any version of mod 'Y' (y), but a matching version is present"
        pattern: r"Mod '([^']+)' \(([^)]+)\) \S+ is incompatible with [^']*'([^']+)'",
        explain: |c, text| {
            // Often the clash is only an old version of one of them, and the loader says which to update.
            let hint = update_hint(text).map_or_else(String::new, |h| {
                format!(" Fabric Loader suggests: {h}. Otherwise remove one of them.")
            });
            Diagnosis::new(
                format!("{} and {} don't work together", &c[1], &c[3]),
                if hint.is_empty() {
                    format!(
                        "{} doesn't work alongside {}. Remove one of them.",
                        &c[1], &c[3]
                    )
                } else {
                    format!(
                        "{} doesn't work alongside this version of {}.{hint}",
                        &c[1], &c[3]
                    )
                },
                vec![c[1].to_owned(), c[3].to_owned()],
            )
        },
    },
    Rule {
        // Fabric's resolver also logs this in English whatever language the game runs in (the quoted
        // "Mod 'X' ... requires" lines above are translated, and then match nothing):
        //  "Reason: [HARD_DEP nuit_interop 1.0.0-beta.5 {depends nuit @ [1.0.0-beta.5]}, HARD_DEP macos_fix 1.16+1.21.11 {depends minecraft @ [<26.1]}]"
        //  "Fix: add [], remove [], replace [[nuit 1.0.0-beta.4] -> add:nuit 1.0.0-beta.5 ([[1.0.0-beta.5,1.0.0-beta.5]])]"
        pattern: r"HARD_DEP(?:_NO_CANDIDATE)? \S+ \S+ \{depends \S+ @",
        explain: |_, text| resolution_log(text),
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
        // Fabric: "- Mod 'Better Block Entities' (betterblockentities) 1.2 requires version 0.8.12 or later of mod 'Sodium' (sodium), but only the wrong version is present: 0.6.13+mc1.21.1!"
        // (the Minecraft and Fabric Loader versions are handled above): the other mod is simply too old.
        pattern: r"Mod '([^']+)' \(([^)]+)\) \S+ requires (?:any version|version) (.+?) of (?:mod )?'([^']+)' \(([^)]+)\), but only the wrong version is present: ([^!\s]+)",
        explain: |c, text| {
            let hint = update_hint(text)
                .map_or_else(String::new, |h| format!(" Fabric Loader suggests: {h}."));
            Diagnosis::new(
                format!("{} is too old for {}", &c[4], &c[1]),
                format!(
                    "{} needs {} of {}, but this instance has {}. Update {} (from the instance's Mods page), or remove {}.{hint}",
                    &c[1], &c[3], &c[4], &c[6], &c[4], &c[1]
                ),
                vec![c[1].to_owned(), c[4].to_owned()],
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
        // Fabric's root injection failure owns the blame, ahead of outer Mixin errors.
        pattern: crate::crash_conflicts::EXPLICIT_MIXIN_FAILURE_PATTERN,
        explain: |_, text| {
            crate::crash_conflicts::diagnose_mixin(text, &[]).unwrap_or_else(Diagnosis::none)
        },
    },
    Rule {
        // Fabric: "Mixin apply for mod sodium failed sodium.mixins.json:..."
        pattern: crate::crash_conflicts::FABRIC_MIXIN_APPLY_FAILURE,
        explain: broken_mod,
    },
    Rule {
        pattern: crate::crash_conflicts::MIXIN_APPLY_FAILURE,
        explain: broken_mod,
    },
    Rule {
        // Forge/NeoForge and generic Mixin failures often name only the config.
        pattern: crate::crash_conflicts::MIXIN_CONFIG_PATTERN,
        explain: |_, text| {
            crate::crash_conflicts::diagnose_mixin(text, &[]).unwrap_or_else(Diagnosis::none)
        },
    },
    Rule {
        pattern: r"MixinTransformerError|MixinPreProcessorStandard|MixinApplicatorStandard",
        explain: |_, text| {
            if crate::crash_conflicts::preprocessing_class_failure(text) {
                crate::crash_conflicts::diagnose_mixin(text, &[]).unwrap_or_else(|| {
                    Diagnosis::new(
                        "A mod's mixin failed to apply",
                        "A mod's mixin failed while resolving a class. Update your mods, or report this crash to the mod author.",
                        Vec::new(),
                    )
                })
            } else {
                Diagnosis::none()
            }
        },
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
        pattern: OUT_OF_MEMORY,
        explain: |_, text| {
            let detail = match custom_max_memory_mb(text) {
                Some(mb) => format!(
                    "Custom JVM arguments limit Minecraft's heap to {mb} MB. Remove the custom -Xmx flags to use the launcher's memory setting, or raise the last one if your computer has enough memory."
                ),
                None => "Give it more memory in Settings → Memory (or the instance's own setting), or use fewer or lighter mods and resource packs.".into(),
            };
            Diagnosis::new("Minecraft ran out of memory", detail, Vec::new())
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
        pattern: r"Suspected Mods?:([^\r\n]*)",
        explain: |c, text| {
            let mut mods = Vec::new();
            let first = c.get(1).expect("suspected mods capture");
            for (index, line) in std::iter::once(first.as_str())
                .chain(
                    text[c.get(0).expect("suspected mods match").end()..]
                        .lines()
                        .skip(1),
                )
                .enumerate()
            {
                let entry = line.trim();
                // The header may be empty; subsequent blank lines end the list.
                if (index != 0 && (entry.is_empty() || !line.starts_with(char::is_whitespace)))
                    || entry.starts_with("Stacktrace:")
                {
                    break;
                }
                if entry.starts_with("at ") {
                    continue;
                }
                // Forge entries include ", Version: ..."; that comma isn't a separator.
                let entries: Vec<&str> = if entry.contains(", Version:") {
                    vec![entry]
                } else {
                    entry.split(',').collect()
                };
                for name in entries {
                    let name = name.trim();
                    if !name.is_empty()
                        && !name.eq_ignore_ascii_case("none")
                        && !name.contains("Unknown")
                        && !crate::crash_suspect::is_base_entry(name)
                        && !mods.iter().any(|m| m == name)
                    {
                        mods.push(name.to_owned());
                    }
                }
            }
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
        // A class of the game's or loader's own libraries can't be found. Damaged or unreadable
        // files are possible, but the exception alone does not prove corruption or low memory.
        // "NoClassDefFoundError: org/spongepowered/asm/mixin/throwables/MixinError" from Fabric's
        // Mixin bootstrap, before any mod runs.
        pattern: r"(?:NoClassDefFoundError|ClassNotFoundException):? ((?:org[./]spongepowered[./]asm|org[./]objectweb[./]asm|net[./]fabricmc[./]loader|org[./]quiltmc[./]loader|cpw[./]mods|net[./]minecraftforge[./]fml|net[./]neoforged[./]fml|org[./]lwjgl|com[./]mojang[./](?:blaze3d|authlib|brigadier|datafixers|logging|serialization))[\w./$]*)",
        explain: |c, _| Diagnosis {
            repair: true,
            ..Diagnosis::new(
                "A game library couldn't be loaded",
                format!(
                    "Part of Minecraft or its mod loader couldn't be read ({}). The next time you press Play, Arctic checks the game's files (fetching fresh copies of damaged ones) and starts this instance without its start-up speed-ups. If it happens again, please send the report.",
                    c[1].replace('/', ".")
                ),
                Vec::new(),
            )
        },
    },
    Rule {
        pattern: r"(?:NoClassDefFoundError|ClassNotFoundException):[ \t]+(net[./]fabricmc[./]fabric[./]api[./][\w./$]+)",
        explain: missing_fabric_api_class,
    },
    Rule {
        pattern: r"java\.lang\.(?:NoClassDefFoundError|ClassNotFoundException):[ \t]+([\w$]+(?:[./][\w$]+)+)",
        explain: missing_recommended_dependency,
    },
    Rule {
        pattern: LINKAGE_ERROR,
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

fn missing_fabric_api_class(c: &regex::Captures, text: &str) -> Diagnosis {
    let class = c[1].replace('/', ".");
    let package = class.trim_start_matches("net.fabricmc.fabric.api.");
    let module = match package {
        p if p.starts_with("client.event.lifecycle.v1.")
            || p.starts_with("event.lifecycle.v1.") =>
        {
            Some(("lifecycle events", "fabric-lifecycle-events-v1"))
        }
        p if p.starts_with("client.keybinding.v1.") => {
            Some(("key binding API", "fabric-key-binding-api-v1"))
        }
        p if p.starts_with("client.networking.v1.") || p.starts_with("networking.v1.") => {
            Some(("networking API", "fabric-networking-api-v1"))
        }
        "event.Event" | "event.EventFactory" => Some(("base API", "fabric-api-base")),
        _ => None,
    };
    // The current exception's entrypoint names the provider. The mod inventory may contain many
    // unrelated mods, so use it only to look up the name of that provider.
    static ENTRYPOINT: OnceLock<Regex> = OnceLock::new();
    let entrypoint = ENTRYPOINT.get_or_init(|| {
        Regex::new(r"Could not execute entrypoint stage '[^']+'[^\r\n]*?provided by '([\w-]+)'")
            .unwrap()
    });
    let before = &text[..c.get(0).unwrap().start()];
    let context = before
        .rsplit("\n[")
        .next()
        .unwrap_or(before)
        .rsplit("\n\n")
        .next()
        .unwrap_or(before)
        .rsplit("\r\n\r\n")
        .next()
        .unwrap_or(before);
    let id = entrypoint
        .captures_iter(context)
        .last()
        .and_then(|m| m.get(1))
        .map(|m| m.as_str());
    let performance_name = id.and_then(crate::mods::performance::performance_mod_name);
    let name = performance_name.map(str::to_owned).or_else(|| {
        let id = id?;
        Some(
            crate::crash_conflicts::from_crash_report(text)
                .into_iter()
                .find(|m| m.id == id)
                .map_or_else(|| id.to_owned(), |m| m.name),
        )
    });
    let name = name.as_deref().unwrap_or("A mod");
    let title = module.map_or_else(
        || format!("{name} needs Fabric API"),
        |(name_of_module, _)| format!("{name} needs Fabric API's {name_of_module}"),
    );
    let mut detail = module.map_or_else(
        || format!("{name} needs {class}. Fabric API is missing or too old to provide it."),
        |(name_of_module, id)| {
            format!(
                "{name} needs Fabric API's {name_of_module} module ({id}), which is missing or too old."
            )
        },
    );
    let vanilla = text.lines().any(|line| {
        line.starts_with("──── Launching Vanilla (")
            || (line.contains("--gameDir")
                && line
                    .replace('\\', "/")
                    .contains("/instances/vanilla/minecraft"))
    });
    if performance_name.is_some() && vanilla {
        detail.push_str(&format!(
            " Arctic installed {name} through Performance for this Vanilla instance."
        ));
    }
    Diagnosis::new(title, detail, id.into_iter().map(str::to_owned).collect())
}

fn related_packages(a: &str, b: &str, shared_depth: usize) -> bool {
    let mut a = a.split('.');
    let mut b = b.split('.');
    for depth in 0..shared_depth {
        match (a.next(), b.next()) {
            (Some(a), Some(b)) if a == b => {}
            (None, _) | (_, None) => return depth > 0,
            _ => return false,
        }
    }
    true
}

fn missing_entrypoint_library(text: &str, at: usize, mods_dir: Option<&Path>) -> Option<Diagnosis> {
    static CLASS: OnceLock<Regex> = OnceLock::new();
    let class = CLASS
        .get_or_init(|| {
            Regex::new(r"^java\.lang\.(?:NoClassDefFoundError|ClassNotFoundException):[ \t]+([\w$]+(?:[./][\w$]+)+)(?:\s|$)").unwrap()
        })
        .captures(&text[at..])?[1]
        .replace('/', ".");
    let (package, _) = class.rsplit_once('.')?;
    if package != "com.death.core" && !package.starts_with("com.death.core.") {
        return None;
    }

    // Walk back within the same throwable. A separate log record, blank line or suppressed
    // exception must not lend its entrypoint to an unrelated missing class.
    static ENTRYPOINT: OnceLock<Regex> = OnceLock::new();
    let entrypoint = ENTRYPOINT.get_or_init(|| {
        Regex::new(r"Could not execute entrypoint stage '[^']+'[^\r\n]*?provided by '([\w.-]+)' at '([\w$]+(?:\.[\w$]+)+)(?:::[\w$]+)?'").unwrap()
    });
    fn trace_line(line: &str) -> &str {
        line.strip_prefix(VANILLA_CRASH_TRACE)
            .unwrap_or(line)
            .trim()
    }
    let mut before = text[..at].rsplit('\n');
    if trace_line(before.next()?) != "Caused by:" {
        return None;
    }
    let header = before.map(trace_line).find(|line| {
        line.contains("Could not execute entrypoint stage ")
            || !["at ", "Caused by:", "... "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
    })?;
    let owner = entrypoint.captures(header)?;
    let id = &owner[1];
    let (owner_package, _) = owner[2].rsplit_once('.')?;
    // A shared top-level domain such as "com" is too broad to identify a mod.
    if related_packages(package, owner_package, 2) {
        return None;
    }

    let known = mods_dir.map_or_else(Vec::new, crate::crash_suspect::scan);
    if known
        .iter()
        .flat_map(|m| &m.packages)
        .any(|p| related_packages(package, p, 3))
    {
        return None;
    }

    // Reports can identify a plausible provider by mod ID even when its jar is unavailable.
    let reported = crate::crash_conflicts::from_crash_report(text);
    let startup = compact_mod_inventory(text, text);
    let uploaded = text
        .split_once("---- full mod list (id version) ----\n")
        .map_or("", |(_, tail)| {
            tail.split("\n---- ").next().unwrap_or_default()
        });
    // The trace identifies who needed the class. An inventory is needed to infer absence.
    if reported.is_empty()
        && startup.is_empty()
        && uploaded.trim().is_empty()
        && !mods_dir.is_some_and(Path::is_dir)
    {
        return None;
    }
    if reported
        .iter()
        .map(|m| m.id.as_str())
        .chain(known.iter().map(|m| m.id.as_str()))
        .chain(
            startup
                .lines()
                .chain(uploaded.lines())
                .filter_map(|line| line.split_once(' ').map(|(id, _)| id)),
        )
        .any(|mod_id| {
            mod_id
                .replace(['-', '_', '.'], "")
                .eq_ignore_ascii_case("deathcore")
        })
    {
        return None;
    }
    let name = known
        .iter()
        .find(|m| m.id == id)
        .map(|m| m.name.as_str())
        .filter(|name| *name != id)
        .or_else(|| {
            reported
                .iter()
                .find(|m| m.id == id)
                .map(|m| m.name.as_str())
        })
        .unwrap_or(id);
    Some(Diagnosis::new(
        format!("{name} needs DeathCore"),
        format!(
            "{name} couldn't load {class}. Install a compatible version of DeathCore, or remove {name}."
        ),
        vec![id.to_owned()],
    ))
}

fn missing_recommended_dependency(class: &regex::Captures, text: &str) -> Diagnosis {
    static WARNING: OnceLock<Regex> = OnceLock::new();
    static ENTRYPOINT: OnceLock<Regex> = OnceLock::new();
    let warning = WARNING.get_or_init(|| {
        Regex::new(r"Mod '([^\r\n]+?)' \(([\w.-]+)\) [^\r\n]*?\b(?:recommends|suggests) (?:[^\r\n]*?\bof )?(?:(?:mod )?'([^\r\n]+?)' \()?([\w.-]+)\)?, which is missing!").unwrap()
    });
    let entrypoint = ENTRYPOINT.get_or_init(|| {
        Regex::new(r"Could not execute entrypoint stage [^\r\n]*?provided by '([\w.-]+)'").unwrap()
    });
    let before = &text[..class.get(0).unwrap().start()];
    let owner = entrypoint
        .captures_iter(before)
        .last()
        .and_then(|c| c.get(1))
        .map(|id| id.as_str());
    let (package, _) = class[1].rsplit_once(['/', '.']).unwrap();
    let Some(missing) = warning
        .captures_iter(text)
        .filter(|c| {
            let segment = match &c[4] {
                "fabric-language-kotlin" => "kotlin".to_owned(),
                id => id.replace(['-', '_'], ""),
            };
            package
                .split(['/', '.'])
                .any(|part| part.replace('_', "").eq_ignore_ascii_case(&segment))
        })
        .min_by_key(|c| owner != Some(&c[2]))
    else {
        return Diagnosis::none();
    };
    let name = &missing[1];
    let dependency = missing.get(3).map_or_else(
        || match &missing[4] {
            "modernui" => "Modern UI",
            "fabric-language-kotlin" => "Fabric Language Kotlin",
            "cloth-config" => "Cloth Config",
            id => id,
        },
        |display| display.as_str(),
    );
    Diagnosis::new(
        format!("{name} needs {dependency}"),
        format!("Install {dependency} or remove {name}."),
        vec![name.to_owned(), dependency.to_owned()],
    )
}

fn split_package(a: &str, b: &str, package: &str) -> Diagnosis {
    Diagnosis::new(
        "Two mods contain the same code",
        format!(
            "{a} and {b} both contain package {package} and can't be loaded together. Remove or update one of them. Usually one is a duplicate or bundles another mod."
        ),
        vec![a.to_owned(), b.to_owned()],
    )
}

/// The mod list of a Fabric crash report names each mod with its version, and a build for another
/// Minecraft version says so ("1.8.3+mc1.21.4"): (the mod's version, the Minecraft version it is
/// built for, the game's) when that is more than a patch release away from the game.
fn built_for_another_game(id: &str, text: &str) -> Option<(String, String, String)> {
    let game = Regex::new(r"(?m)^\s+minecraft: Minecraft (\S+)")
        .ok()?
        .captures(text)?[1]
        .to_owned();
    let entry = Regex::new(&format!(
        r"(?m)^\s+{}: .*? (\S+\+mc([0-9][0-9.]*))\s*$",
        regex::escape(id)
    ))
    .ok()?;
    let found = entry.captures(text)?;
    let (version, built_for) = (found[1].to_owned(), found[2].to_owned());
    let parts = |v: &str| -> Vec<u32> { v.split('.').filter_map(|p| p.parse().ok()).collect() };
    let (have, want) = (parts(&game), parts(&built_for));
    let at = |p: &[u32], i: usize| p.get(i).copied().unwrap_or(0);
    // 1.21 and 1.21.1 share their mods; 1.21.4 does not.
    let other = at(&have, 0) != at(&want, 0)
        || at(&have, 1) != at(&want, 1)
        || at(&have, 2).abs_diff(at(&want, 2)) > 1;
    other.then_some((version, built_for, game))
}

fn broken_mod(c: &regex::Captures, text: &str) -> Diagnosis {
    let before = &text[..c.get(0).expect("mixin failure match").start()];
    let prefix = before.rsplit('\n').next().unwrap_or(before);
    if warning_log_line(prefix) {
        return Diagnosis::none();
    }
    let id = &c[1];
    if let Some((version, built_for, game)) = built_for_another_game(id, text) {
        return Diagnosis::new(
            format!("{id} is for another Minecraft version"),
            format!(
                "{id} {version} is built for Minecraft {built_for}, but this game is {game}. Get its {game} version (the instance's Mods page, Look for updates), or remove it."
            ),
            vec![id.to_owned()],
        );
    }
    Diagnosis::new(
        format!("{id} failed to load"),
        format!(
            "{id} couldn't hook into the game: it's likely made for another Minecraft version or clashes with another mod. Update or remove it."
        ),
        vec![id.to_owned()],
    )
}

pub(crate) fn warning_log_line(line: &str) -> bool {
    static WARNING: OnceLock<Regex> = OnceLock::new();
    WARNING
        .get_or_init(|| {
            Regex::new(r"^(?:\[[^\]\r\n]*\][ \t]*)*\[(?:[^\]\r\n]*/)?WARN(?:ING)?\]").unwrap()
        })
        .is_match(line.trim_start())
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

/// Fabric's missing-dependency reports give the id of the requiring mod.
pub(crate) fn fabric_missing_dependencies(text: &str) -> impl Iterator<Item = (&str, &str)> {
    static NO_CANDIDATE: OnceLock<Regex> = OnceLock::new();
    let no_candidate = NO_CANDIDATE
        .get_or_init(|| Regex::new(r"HARD_DEP_NO_CANDIDATE (\S+) \S+ \{depends (\S+) @").unwrap());
    compiled()[0]
        .captures_iter(text)
        .filter_map(|c| Some((c.get(2)?.as_str(), c.get(3)?.as_str())))
        .chain(
            resolver_causes(text)
                .flat_map(|line| no_candidate.captures_iter(line))
                .filter_map(|c| Some((c.get(1)?.as_str(), c.get(2)?.as_str()))),
        )
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

/// Real replacement versions from Fabric's resolver, excluding bare-number placeholders.
fn resolver_updates(text: &str) -> Vec<(String, String)> {
    let re = Regex::new(r"\[([^\s\[\]]+) [^\s\]]+\] -> add:(\S+) (\S+) (\S+)")
        .expect("resolver update pattern is valid");
    re.captures_iter(text)
        .filter(|m| {
            let version = &m[3];
            let lower = m[4]
                .trim_start_matches(['(', '['])
                .split(',')
                .next()
                .unwrap_or_default();
            // Fabric can suggest a bare number (e.g. "1") instead of a concrete version.
            // Keep actual single-component versions, whose lower bound agrees with the hint.
            !(version.chars().all(|c| c.is_ascii_digit())
                && lower != version
                && lower.chars().any(|c| !c.is_ascii_digit()))
        })
        .map(|m| (m[1].to_owned(), m[3].to_owned()))
        .collect()
}

/// The loader's own suggestion from its English "Fix:" line: "update sodium to 0.8.12".
fn update_hint(text: &str) -> Option<String> {
    let updates: Vec<String> = resolver_updates(text)
        .into_iter()
        .map(|(id, version)| format!("update {id} to {version}"))
        .collect();
    (!updates.is_empty()).then(|| updates.join(", "))
}

/// A game that stopped without writing a single line (only the launcher's banner is there): what it
/// usually means. `None` when the game did write something (then its log has the answer).
pub fn silent_exit(text: &str, code: i32) -> Option<Diagnosis> {
    let wrote_something = text
        .lines()
        .map(str::trim)
        .any(|l| !l.is_empty() && !l.starts_with("────") && !l.starts_with("[Arctic diagnostics]"));
    if wrote_something {
        return None;
    }
    Some(Diagnosis::new(
        format!(
            "The game stopped before it wrote anything (exit {})",
            exit_description(code)
        ),
        format!(
            "Java ended right after it started (exit code {code}) without a message. Usually antivirus or a security tool blocked Java or a game file, the Java runtime is damaged, or the memory setting is more than the computer can give. Try: add the Arctic folder to your antivirus' exceptions, lower Maximum memory in Settings, and use Repair on the instance."
        ),
        Vec::new(),
    ))
}

/// The final Reason lists failed constraints; Immediate reason can also include satisfied ones.
/// Keep standalone constraints usable when the final Reason wasn't captured in the log.
fn resolver_causes(text: &str) -> impl Iterator<Item = &str> {
    let has_reason = text.lines().any(|line| line.contains("Reason: ["));
    text.lines()
        .filter(move |line| !has_reason || !line.contains("Immediate reason: ["))
}

/// What Fabric's English resolver lines say is wrong with the mods (see the rule that calls this).
fn resolution_log(text: &str) -> Diagnosis {
    let updates = resolver_updates(text);
    let mut for_other_minecraft: Vec<String> = Vec::new();
    let mut needs_newer_loader = false;
    let mut missing: Vec<String> = Vec::new();
    let mut involved: Vec<String> = Vec::new();
    if let Ok(re) =
        Regex::new(r"HARD_DEP(?:_NO_CANDIDATE)? (\S+) \S+ \{depends (\S+) @ ([^}\r\n]+)\}")
    {
        for m in resolver_causes(text).flat_map(|line| re.captures_iter(line)) {
            match &m[2] {
                "minecraft" if !for_other_minecraft.contains(&m[1].to_owned()) => {
                    for_other_minecraft.push(m[1].to_owned());
                }
                "minecraft" => {}
                "fabricloader" => needs_newer_loader = true,
                // A concrete replacement identifies an installed dependency with the wrong version.
                id if updates.iter().any(|(updated, _)| updated == id) => {}
                _ => {
                    let detail = format!(
                        "{} needs {} (version {}), which isn't installed.",
                        &m[1],
                        &m[2],
                        m[3].trim()
                    );
                    if !missing.contains(&detail) {
                        missing.push(detail);
                    }
                    for id in [&m[1], &m[2]] {
                        if !involved.iter().any(|name| name == id) {
                            involved.push(id.to_owned());
                        }
                    }
                }
            }
        }
    }
    let have = Regex::new(r"Loading Minecraft (\S+) with")
        .ok()
        .and_then(|re| re.captures(text).map(|c| c[1].to_owned()))
        .unwrap_or_else(|| "this one".to_owned());
    let also = if updates.is_empty() {
        String::new()
    } else {
        let updates: Vec<String> = updates
            .iter()
            .map(|(id, version)| format!("{id} to {version}"))
            .collect();
        format!(" Also update {}.", updates.join(", "))
    };
    if !for_other_minecraft.is_empty() {
        return Diagnosis {
            fix: Some(Fix::DisableMods(for_other_minecraft.clone())),
            ..Diagnosis::new(
                format!(
                    "{} mod(s) are for another Minecraft version",
                    for_other_minecraft.len()
                ),
                format!(
                    "These mods are made for another Minecraft version than {have}: {}. They are probably left over from another version (an instance copied or switched to a newer game). Switch them off, or update them from the instance's Mods page.{also}",
                    for_other_minecraft.join(", ")
                ),
                for_other_minecraft,
            )
        };
    }
    if needs_newer_loader {
        return Diagnosis {
            fix: Some(Fix::UpdateLoader),
            ..Diagnosis::new(
                "Fabric Loader is too old for these mods",
                format!(
                    "Some mods need a newer Fabric Loader. Update it in the instance's settings (Loader), or remove those mods.{also}"
                ),
                Vec::new(),
            )
        };
    }
    if !missing.is_empty() {
        return Diagnosis::new("A mod dependency is missing", missing.join(" "), involved);
    }
    if !updates.is_empty() {
        return Diagnosis::new(
            "Some mods need updating together",
            format!("Mods in this instance don't match each other's versions:{also}"),
            Vec::new(),
        );
    }
    Diagnosis::new("", "", Vec::new())
}

/// Explain a crash from the game log and/or crash report text.
pub fn diagnose(text: &str) -> Option<Diagnosis> {
    diagnose_match(text, None).map(|(diagnosis, _)| diagnosis)
}

pub(crate) fn diagnose_with_java(
    text: &str,
    java: Option<&str>,
    mods_dir: &Path,
    mods: &[crate::crash_conflicts::InstalledMod],
) -> Option<Diagnosis> {
    diagnose_rules(text, java, Some(mods_dir), mods)
        .or_else(|| hang_diagnosis(text, Some(mods_dir)))
        .map(|(diagnosis, _)| diagnosis)
}

const VANILLA_CRASH: &str = "[Arctic diagnostics] vanilla crash";
const VANILLA_CRASH_TRACE: &str = "[Arctic diagnostics] vanilla crash trace ";
const DISK_FULL_TITLE: &str = "Your disk is full";
pub(crate) const SAVE_FAILED_TITLE: &str =
    "Minecraft crashed while starting and couldn't save its crash report";

pub(crate) fn disk_full_diagnosis(text: &str) -> Option<(Diagnosis, String)> {
    static DISK_FULL: OnceLock<Regex> = OnceLock::new();
    static WINDOWS_DRIVE: OnceLock<Regex> = OnceLock::new();
    let disk_full = DISK_FULL.get_or_init(|| {
        Regex::new(
            r"(?i)there is not enough space on the disk|no space left on device|\bos error (?:112|28)\b|\bIOException:[ \t]*disk full\b",
        )
        .unwrap()
    });
    let drive_in = |line| {
        let drives = WINDOWS_DRIVE.get_or_init(|| Regex::new(r"\b[A-Za-z]:[/\\]").unwrap());
        drives
            .find(line)
            .map(|drive| drive.as_str()[..2].to_ascii_uppercase())
    };

    let mut previous = "";
    let mut evidence = None;
    let mut drive = None;
    for line in text.lines() {
        if disk_full.is_match(line) {
            evidence.get_or_insert(line);
            // Java can log the failed write's path on the line above the exception.
            drive = drive_in(line).or_else(|| {
                (previous.contains("Could not save crash report to ")
                    || previous.starts_with("[Arctic diagnostics] vanilla crash save failed: "))
                .then(|| drive_in(previous))
                .flatten()
            });
            if drive.is_some() {
                evidence = Some(line);
                break;
            }
        }
        previous = line;
    }
    let evidence = head_of(evidence?.trim(), 2048);
    let drive = drive.map_or_else(
        || "the drive".to_owned(),
        |drive| format!("the {drive} drive"),
    );
    Some((
        Diagnosis::new(
            DISK_FULL_TITLE,
            format!(
                "Minecraft couldn't write files because {drive} is out of space. Free up some space and try again."
            ),
            Vec::new(),
        ),
        evidence.to_owned(),
    ))
}

pub(crate) fn vanilla_crash_save_failed(text: &str, code: i32) -> Option<&str> {
    // POSIX process exit statuses carry only the low byte of Java's -2.
    if !matches!(code, -2 | 254) {
        return None;
    }
    text.lines().find(|line| {
        line.strip_prefix("[Arctic diagnostics] exit caller vanilla ")
            .and_then(|frame| frame.split('(').next())
            .is_some_and(|method| {
                matches!(
                    method.rsplit('/').next(),
                    Some(
                        "net.minecraft.client.Minecraft.crash"
                            | "net.minecraft.class_310.method_1565"
                    )
                )
            })
    })
}

pub(crate) fn logged_vanilla_crash_save_failure(text: &str) -> Option<&str> {
    let code = text.lines().rev().find_map(|line| {
        line.strip_prefix('(')
            .unwrap_or(line)
            .strip_prefix("exit code ")
            .or_else(|| {
                line.strip_prefix("[Arctic diagnostics] process ended after ")?
                    .split_once("; exit code ")
                    .map(|(_, code)| code)
            })
            .or_else(|| {
                line.strip_prefix("[Arctic diagnostics] exit Runtime.exit() called with status: ")
            })?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    })?;
    vanilla_crash_save_failed(text, code)
}

fn vanilla_crash_trace(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .filter_map(|line| line.strip_prefix(VANILLA_CRASH_TRACE))
}

pub(crate) fn captured_vanilla_crash(text: &str) -> Option<String> {
    let mut lines = vanilla_crash_trace(text).peekable();
    lines.peek()?;
    // The captured exception has the same provenance as a written report's exception.
    let mut report = String::from("---- Minecraft Crash Report ----\n\n");
    for line in lines {
        report.push_str(line);
        report.push('\n');
    }
    if let Some((startup, _)) = fabric_loader_startup(text) {
        report.push_str(startup);
        report.push('\n');
    }
    Some(report)
}

pub(crate) fn vanilla_crash_diagnosis(text: &str, cause: Option<Diagnosis>) -> Diagnosis {
    if let Some(mut cause) = cause {
        cause.detail = format!("{}\n\n{}", cause.title, cause.detail);
        cause.title = SAVE_FAILED_TITLE.into();
        return cause;
    }
    let detail = vanilla_crash_trace(text).next().map_or_else(
        || "Minecraft exited from its crash handler before it could save a report.".into(),
        |error| scrub(head_of(error, 2048), &Own::this_pc()),
    );
    Diagnosis::new(SAVE_FAILED_TITLE, detail, Vec::new())
}

fn quilt_load_errors() -> &'static Regex {
    static ERRORS: OnceLock<Regex> = OnceLock::new();
    ERRORS.get_or_init(|| Regex::new(
        r"(?:Duplicate mandatory mod ids \[(?P<ids>[^\]\r\n]+)\]|[0-9]+ mods? requires version (?P<range>\[[^\r\n]+?\)) of fabricloader, which is missing!)(?P<files>(?:[ \t\r\n]+-[ \t]+[^\r\n]+)+)"
    ).unwrap())
}

/// Quilt reports resolver failures without throwing an exception, then calls System.exit(1).
/// Match both error kinds together so the first actual error in the report wins.
fn quilt_load_error(text: &str) -> Option<(Diagnosis, String)> {
    let c = quilt_load_errors().captures(text)?;
    let files: Vec<String> = c["files"]
        .lines()
        .flat_map(|line| line.trim().trim_start_matches("- ").split(" - "))
        .filter_map(|path| {
            let file = path.trim().rsplit(['/', '\\']).next()?;
            (!file.is_empty()).then(|| file.to_owned())
        })
        .collect();
    let diagnosis = if let Some(ids) = c.name("ids") {
        let ids: Vec<String> = ids
            .as_str()
            .split(',')
            .map(|id| id.trim().to_owned())
            .collect();
        let name = &ids[0];
        Diagnosis::new(
            format!("Duplicate mod: {name} installed twice"),
            format!(
                "{name} is installed twice ({}). Remove one of these jars from the instance's Mods folder.",
                files.join(", ")
            ),
            ids,
        )
    } else {
        Diagnosis::new(
            "Fabric Loader is too old for these mods",
            format!(
                "{} need Fabric Loader {}. Quilt's Fabric compatibility version does not meet this requirement. Update Quilt Loader to a version that provides the required Fabric Loader version, or use compatible versions of these mods.",
                files.join(", "),
                &c["range"]
            ),
            files,
        )
    };
    Some((diagnosis, head_of(c.get(0)?.as_str(), 2048).to_owned()))
}

/// The report's primary exception and cause chain, before the detailed walkthrough/inventory.
pub(crate) fn crash_exception(text: &str) -> Option<&str> {
    let start = text.rfind("---- Minecraft Crash Report ----")?;
    let report = &text[start..];
    static EXCEPTION: OnceLock<Regex> = OnceLock::new();
    let exception =
        EXCEPTION.get_or_init(|| Regex::new(r"(?m)^[\w.$]+(?:Exception|Error)(?::|\r?$)").unwrap());
    let start = exception.find(report)?.start();
    let stack = &report[start..];
    let end = stack
        .find("\n\n")
        .into_iter()
        .chain(stack.find("\r\n\r\n"))
        .min()
        .unwrap_or(stack.len());
    Some(&stack[..end])
}

/// Some mods explicitly report that their failure followed a failed mod load.
pub(crate) fn secondary_mod_failure(line: &str) -> bool {
    static SECONDARY: OnceLock<Regex> = OnceLock::new();
    SECONDARY
        .get_or_init(|| {
            Regex::new(r"(?i)\b(?:another|other) mod (?:has )?failed (?:to (?:load|initialize)\b|first\b)|\bdue to (?:a )?previous mod loading (?:error|failure)\b").unwrap()
        })
        .is_match(line)
}

pub(crate) fn secondary_stack(stack: &str) -> bool {
    stack.lines().enumerate().any(|(i, line)| {
        (i == 0 || line.trim_start().starts_with("Caused by:")) && secondary_mod_failure(line)
    })
}

/// Forge can already have rejected the mod list before the client throws its fatal exception.
pub(crate) fn forge_dependency_diagnosis(text: &str) -> Option<(Diagnosis, String)> {
    const MANDATORY: &str = "Missing or unsupported mandatory dependencies";
    static PROVIDER: OnceLock<Regex> = OnceLock::new();
    static DEPENDENCY: OnceLock<Regex> = OnceLock::new();
    let provider_pattern = PROVIDER.get_or_init(|| {
        Regex::new(
            r"\bMod File (.+?) needs language provider ([\w.-]+)(?::(\S+) or above)? to load\b",
        )
        .unwrap()
    });
    let dependency_pattern = DEPENDENCY.get_or_init(|| {
        Regex::new(r"Mod ID: '([^']+)', Requested by: '([^']+)', Expected range: '([^']*)', Actual version: '([^']*)'").unwrap()
    });
    let mut summary = None;
    let mut dependency = None;
    for line in tail_of(text, MAX_TEXT).lines().map(str::trim_start) {
        if line.contains(" needs language provider ")
            && !warning_log_line(line.trim_start_matches("matched: "))
            && let Some(c) = provider_pattern.captures(line)
        {
            let file = c[1].trim().rsplit(['/', '\\']).next().unwrap_or(&c[1]);
            let provider = match &c[2] {
                "kotlinforforge" => "Kotlin for Forge",
                id => id,
            };
            let required = c.get(3).map_or_else(
                || provider.to_owned(),
                |version| format!("{provider} {} or newer", version.as_str()),
            );
            return Some((
                Diagnosis::new(
                    format!("{file} needs {required}"),
                    format!("Install or update {required} in this instance."),
                    vec![file.to_owned(), c[2].to_owned()],
                ),
                head_of(line, 2048).to_owned(),
            ));
        }
        if dependency.is_none()
            && line.contains("Mod ID: ")
            && !warning_log_line(line.trim_start_matches("matched: "))
        {
            dependency = dependency_pattern.captures(line).map(|c| (c, line));
        }
        if line.trim_end().trim_end_matches(':').ends_with(MANDATORY)
            && !warning_log_line(line.trim_start_matches("matched: "))
        {
            summary.get_or_insert(line);
        }
    }
    let (diagnosis, line) = if let Some((c, line)) = dependency {
        let (id, requested_by, expected, actual) = (&c[1], &c[2], &c[3], &c[4]);
        let (title, detail) = if actual == "[MISSING]" {
            (
                format!("A mod needs {id}"),
                format!("{requested_by} needs {id}, which isn't installed."),
            )
        } else {
            (
                format!("{id} is the wrong version"),
                format!("{requested_by} needs {id} {expected}, but {actual} is installed."),
            )
        };
        (
            Diagnosis::new(title, detail, vec![id.to_owned(), requested_by.to_owned()]),
            line,
        )
    } else {
        (
            Diagnosis::new(
                "A required mod dependency is missing or incompatible",
                "Check the instance log for the required mods and versions, then install or update them.",
                Vec::new(),
            ),
            summary?,
        )
    };
    Some((diagnosis, head_of(line, 2048).to_owned()))
}

fn class_loading_warning(line: &str) -> bool {
    line.contains("Error loading class:")
        && warning_log_line(line.trim_start().trim_start_matches("matched: "))
}

/// Forge reports the failed entries separately from the exception. Only those explicit
/// registry records can identify affected namespaces; applied mixins do not identify a culprit.
pub(crate) fn registry_sync_diagnosis(text: &str) -> Option<(Diagnosis, String)> {
    let stack = crash_exception(text).unwrap_or(text);
    if !stack.lines().any(|line| {
        line.trim_start()
            .starts_with("at net.minecraftforge.registries.ForgeRegistry.sync(")
    }) {
        return None;
    }
    static FAILURE: OnceLock<Regex> = OnceLock::new();
    let failure = FAILURE.get_or_init(|| {
        Regex::new(r"(?i)(?:did not copy to (?:the )?(?:correct|expected) id|\bids?\b.*(?:mismatch|do not match|did not match|out of sync)|mismatch.*registry.*\bids?\b|registry.*sync.*fail)").unwrap()
    });
    let matched = stack.lines().find(|line| {
        (line.contains("Exception:") || line.contains("Error:")) && failure.is_match(line)
    })?;
    static ENTRY: OnceLock<Regex> = OnceLock::new();
    let entry = ENTRY.get_or_init(|| Regex::new(r"\bName: ([a-z0-9_.-]+):[a-z0-9_./-]+").unwrap());
    let evidence = registry_failure_evidence(text);
    let mut mods = Vec::new();
    for line in evidence.lines().filter(|line| registry_id_error(line)) {
        if let Some(c) = entry.captures(line) {
            let id = c[1].to_owned();
            if id != "minecraft" && id != "forge" && !mods.contains(&id) {
                mods.push(id);
            }
        }
    }
    let affected = if mods.is_empty() {
        "The available log does not identify the affected mod entries.".to_owned()
    } else {
        format!(
            "Forge names entries in these mod namespaces: {}. Another mod may have replaced those entries.",
            mods.join(", ")
        )
    };
    Some((
        Diagnosis::new(
            "Forge registry IDs did not sync",
            format!(
                "A mod's registry entries did not get the expected IDs while Forge synchronized registries. This indicates a mod conflict in the pack. {affected} Update the pack, or send this report to the pack author."
            ),
            mods,
        ),
        head_of(matched.trim_end(), 2048).to_owned(),
    ))
}

/// Linkage errors in ordinary log records are often optional integrations failing safely.
/// Restrict them to a crash document or an uncaught/fatal exception block with no later progress.
fn fatal_linkage(text: &str, at: usize) -> bool {
    let before = &text[..at];
    let line_prefix = before.rsplit('\n').next().unwrap_or(before);
    // A Mixin class probe stays a warning even when logs are copied after a crash report.
    if class_loading_warning(line_prefix) {
        return false;
    }
    if line_prefix.starts_with(VANILLA_CRASH_TRACE) {
        return true;
    }
    if before.contains("---- Minecraft Crash Report ----")
        || before.contains("---- crash report ----\n")
        || before.contains("A fatal error has been detected by the Java Runtime Environment")
    {
        return true;
    }
    let start = before.rfind("\n[").map_or(0, |n| n + 1);
    let context = before[start..].to_lowercase();
    let explicit = context.contains("exception in thread")
        || context.contains("uncaught exception")
        || context.contains("/fatal]")
        || context.contains("minecraft has crashed!")
        || context.contains("encountered an unexpected exception")
        || context.contains("reported exception thrown");
    let later_progress = text[at..]
        .lines()
        .skip(1)
        .any(|line| line.contains("/INFO]") || line.contains("/WARN]") || line.contains("/ERROR]"));
    (explicit && !later_progress)
        || (text.trim_start().starts_with("java.lang.")
            && !text.contains("/INFO]")
            && !text.contains("/WARN]")
            && !text.contains("/ERROR]")
            && text.lines().skip(1).all(|line| {
                let line = line.trim();
                line.is_empty()
                    || line.starts_with("at ")
                    || line.starts_with("Caused by:")
                    || line.starts_with("Suppressed:")
                    || line.starts_with("... ")
            })
            && !freeze_evidence(text))
}

const JAVA_FATAL_HEADER: &str =
    "# A fatal error has been detected by the Java Runtime Environment:";

fn java_fatal_error(text: &str) -> Option<(&str, &str)> {
    // HotSpot can print the header to stderr as well as writing the fuller hs_err file.
    // The file is appended after the process output.
    let (start, _) = text
        .rmatch_indices(JAVA_FATAL_HEADER)
        .find(|&(start, _)| start == 0 || text.as_bytes()[start - 1] == b'\n')?;
    let report = &text[start..];
    let report = report.split("\n---- ").next()?;
    let fatal = report
        .lines()
        .skip(1)
        .map(str::trim)
        .find(|line| !line.is_empty() && *line != "#")?;
    fatal.starts_with("# ").then_some((report, fatal))
}

fn native_library_owner<'a>(
    text: &str,
    report: &str,
    frame_path: &str,
    library: &str,
    inventory: &'a str,
) -> Option<&'a str> {
    let mods = || inventory.lines().filter_map(|line| line.split_once(' '));
    let paths = text.replace('\\', "/");
    // Match the full library name. Sibling libraries can differ only by a suffix.
    let path = Regex::new(&format!(
        r"(?:^|[\s/])(?:config|mods)/([^/\r\n]+)/(?:[^\r\n]*/)?{}(?:$|[\s+!\]])",
        regex::escape(library)
    ))
    .ok()?;
    let owner_for_dir = |directory: &str| {
        mods().find_map(|(id, version)| {
            let stem = directory
                .strip_suffix(".jar!")
                .or_else(|| directory.strip_suffix(".jar"));
            (directory == id
                || stem.is_some_and(|stem| stem == id || stem == format!("{id}-{version}")))
            .then_some(id)
        })
    };
    for source in [frame_path, &paths] {
        let mut owners = path
            .captures_iter(source)
            .filter_map(|c| owner_for_dir(c.get(1)?.as_str()));
        if let Some(owner) = owners.next() {
            return owners.all(|other| other == owner).then_some(owner);
        }
    }

    // The first Java frame is the Java side of the native call. Later frames can
    // belong to callers in other mods.
    let frame = report
        .lines()
        .skip_while(|line| !line.starts_with("Java frames:"))
        .nth(1)?;
    let mut words = frame.split_whitespace();
    let method = match words.next()? {
        "j" => words.next()?,
        "J" => words.nth(2)?,
        _ => return None,
    };
    let (method, _) = method.split_once('(')?;
    let (package, _) = method.rsplit_once('.')?;
    let (package, _) = package.rsplit_once('.')?;
    let mut owners = mods().filter_map(|(id, _)| {
        let normalized = id.bytes().filter(u8::is_ascii_alphanumeric);
        package
            .split(['.', '/'])
            .any(|part| {
                part.bytes()
                    .filter(u8::is_ascii_alphanumeric)
                    .map(|b| b.to_ascii_lowercase())
                    .eq(normalized.clone())
            })
            .then_some(id)
    });
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner)
}

fn native_library_diagnosis(text: &str, report: &str, fatal: &str) -> Option<Diagnosis> {
    let signal = fatal.strip_prefix("# ")?.split_whitespace().next()?;
    if !matches!(
        signal,
        "SIGILL" | "SIGSEGV" | "SIGBUS" | "SIGFPE" | "SIGABRT" | "SIGTRAP"
    ) && !signal.starts_with("EXCEPTION_")
    {
        return None;
    }
    let frame = crate::stability::problematic_frame(report)?;
    let frame = frame.strip_prefix("C ")?.trim_start().strip_prefix('[')?;
    let (frame_path, _) = frame.split_once(']')?;
    let (frame_path, _) = frame_path.rsplit_once("+0x")?;
    let frame_path = frame_path.replace('\\', "/");
    let library = frame_path.rsplit('/').next()?;
    if matches!(
        library.to_ascii_lowercase().as_str(),
        "libjvm.so" | "libjvm.dylib" | "libjvm.a" | "jvm.dll"
    ) {
        return None;
    }

    let inventory = compact_mod_inventory(text, text);
    let uploaded_inventory = text
        .split_once("---- full mod list (id version) ----\n")
        .map_or("", |(_, tail)| {
            tail.split("\n---- ").next().unwrap_or_default().trim()
        });
    let inventory = if inventory.is_empty() {
        uploaded_inventory
    } else {
        &inventory
    };
    let owner = native_library_owner(text, report, &frame_path, library, inventory);
    let title = match owner {
        Some(mod_id) => format!("{mod_id} crashed in native library {library}"),
        None => format!("Minecraft crashed in native library {library}"),
    };
    let detail = if matches!(signal, "SIGILL" | "EXCEPTION_ILLEGAL_INSTRUCTION") {
        let cpu = report
            .lines()
            .find_map(|line| line.strip_prefix("Host: "))
            .and_then(|host| host.split(',').next())
            .filter(|cpu| !cpu.is_empty());
        let cpu = cpu.map_or_else(|| "this CPU".to_owned(), |cpu| format!("this CPU ({cpu})"));
        match owner {
            Some(mod_id) => format!(
                "{mod_id}'s native code uses instructions that {cpu} doesn't support. Remove {mod_id} or replace it with a version that supports your CPU."
            ),
            None => format!(
                "{library} uses instructions that {cpu} doesn't support. If it came with a mod, remove that mod or replace it with a version that supports your CPU."
            ),
        }
    } else if let Some(mod_id) = owner {
        format!(
            "The game stopped with {signal} in {mod_id}'s native library. Update {mod_id} or try launching without it."
        )
    } else if let Some((driver, _)) = diagnose_log_rules(report, None, None)
        .filter(|(diagnosis, _)| diagnosis.title == "The graphics driver crashed")
    {
        return Some(Diagnosis::new(
            format!("{} ({library})", driver.title),
            driver.detail,
            Vec::new(),
        ));
    } else {
        format!("The game stopped with {signal} in {library}.")
    };
    Some(Diagnosis::new(
        title,
        detail,
        owner.into_iter().map(str::to_owned).collect(),
    ))
}

pub(crate) fn java_fatal_diagnosis(text: &str) -> Option<(Diagnosis, String)> {
    let (report, fatal) = java_fatal_error(text)?;
    let compiler_thread = report.lines().any(|line| {
        line.trim_start().starts_with("Current thread ")
            && (line.contains("\"C2 CompilerThread") || line.contains("\"C1 CompilerThread"))
    });
    let mut lines = report.lines();
    let compile_task = lines
        .find_map(|line| line.trim_start().strip_prefix("Current CompileTask:"))
        .map(|task| {
            if task.trim().is_empty() {
                lines.next().unwrap_or_default()
            } else {
                task
            }
        });
    let diagnosis = if compiler_thread || compile_task.is_some() {
        let method =
            compile_task.and_then(|task| task.split_whitespace().find(|word| word.contains("::")));
        let task = method.map_or_else(String::new, |method| {
            format!(" Java was compiling {method} when it crashed.")
        });
        Diagnosis::new(
            "Java's JIT compiler crashed",
            format!(
                "A Java runtime bug or unstable hardware can cause this.{task} Clear the instance's custom Java setting to use the Java runtime Arctic recommends for this Minecraft version. If it happens again, test your RAM and turn off any CPU or RAM overclock."
            ),
            Vec::new(),
        )
    } else if let Some(native) = native_library_diagnosis(text, report, fatal) {
        native
    } else {
        diagnose_log_rules(report, None, None).map_or_else(
            || {
                Diagnosis::new(
                    "The Java runtime crashed",
                    "Java reported a fatal error. Try the Java runtime Arctic recommends for this Minecraft version.",
                    Vec::new(),
                )
            },
            |(diagnosis, _)| diagnosis,
        )
    };
    Some((diagnosis, head_of(fatal, 2048).to_owned()))
}

pub(crate) fn native_diagnosis(text: &str) -> Option<(Diagnosis, String)> {
    if text.contains("---- Minecraft Crash Report ----")
        || text.contains("---- crash report ----\n")
        || text.contains("A fatal error has been detected by the Java Runtime Environment")
    {
        return None;
    }
    static EXIT: OnceLock<Regex> = OnceLock::new();
    let re = EXIT.get_or_init(|| Regex::new(r"(?i)exit code -?\d+ \(0x([0-9a-f]{8})").unwrap());
    let matched = re.captures_iter(text).find(|c| {
        u32::from_str_radix(&c[1], 16).is_ok_and(|n| crate::native_crash::is_native(n as i32))
    })?;
    let fault = text
        .lines()
        .find_map(|line| line.strip_prefix("[Arctic diagnostics] native fault: module="));
    let module = fault.map(|s| s.split(';').next().unwrap_or(s));
    let mut title = module.map_or_else(
        || "Minecraft crashed in native code".to_owned(),
        |m| format!("Minecraft crashed in native code ({m})"),
    );
    let owner = fault.and_then(|f| f.split("; bundled mod=").nth(1));
    if let Some(owner) = owner {
        title.push_str(&format!(" — {owner}"));
    }
    let advice = match module.map(str::to_ascii_lowercase) {
        Some(m)
            if m.starts_with("nvoglv")
                || m.starts_with("atio6axx")
                || (m.starts_with("ig") && m.contains("icd")) =>
        {
            "The faulting module is a graphics driver. Update your graphics driver and try without shaders."
        }
        Some(m) if m == "jvm.dll" => {
            "The faulting module is the Java runtime. Try repairing or updating Java."
        }
        Some(_) => {
            "Windows identified the faulting native library. If it is bundled with a mod, update that mod or try without it. The faulting module alone does not prove the root cause."
        }
        None => {
            "The cause is unknown. The native exit code and game logs were captured, but no matching Windows faulting module was available. A driver, overlay, mod native library, or Java runtime may be involved."
        }
    };
    let owner_advice = owner
        .map(|o| format!("The DLL is bundled with {o}; update that mod or try without it. "))
        .unwrap_or_default();
    Some((
        Diagnosis::new(
            title,
            format!(
                "{owner_advice}{advice} No Java fatal-error log or Minecraft crash report was found; fail-fast exits can prevent Java from writing one."
            ),
            Vec::new(),
        ),
        fault.map_or_else(
            || matched[0].to_owned(),
            |f| format!("[Arctic diagnostics] native fault: module={f}"),
        ),
    ))
}

/// A Minecraft mismatch is the cause even when the same report also lists
/// missing libraries for the incompatible mods.
fn fabric_wrong_game(text: &str) -> Option<(Diagnosis, String)> {
    static WRONG_GAME: OnceLock<Regex> = OnceLock::new();
    let re = WRONG_GAME.get_or_init(|| Regex::new(
        r"Mod '([^\r\n]+?)' \(([\w.-]+)\) \S+ requires [^\r\n]+? of (?:mod )?'Minecraft' \(minecraft\), but only the wrong version is present: ([^!\s]+)"
    ).unwrap());
    let mut matches = re.captures_iter(text).peekable();
    let first = matches.peek()?;
    let have = first.get(3)?.as_str();
    let matched = first.get(0)?.as_str();
    let vanilla = text.contains("Launching Vanilla (") || text.contains("-Darctic.brand=vanilla");
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();
    let mut performance = Vec::new();
    for m in matches {
        let (name, id) = (m.get(1)?.as_str(), m.get(2)?.as_str());
        if seen.insert(id) {
            names.push(name.to_owned());
            if vanilla && crate::mods::performance::is_performance_mod(id) {
                performance.push(name);
            }
        }
    }
    let summary = if names.len() == 1 {
        "1 mod is for a different Minecraft version".to_owned()
    } else {
        format!("{} mods are for a different Minecraft version", names.len())
    };
    let mut detail = format!(
        "Minecraft {have} cannot load these mods: {}.",
        names.join(", ")
    );
    if performance.is_empty() {
        detail.push_str(" Switch them off, or update them from the instance's Mods page.");
    } else {
        detail.push_str(&format!(
            " Arctic's Performance mods from another Minecraft version are still in this Vanilla instance: {}.",
            performance.join(", ")
        ));
    }
    Some((
        Diagnosis {
            fix: Some(Fix::DisableMods(names.clone())),
            ..Diagnosis::new(format!("{summary}: {}", names.join(", ")), detail, names)
        },
        head_of(matched, 2048).to_owned(),
    ))
}

fn fabric_loader_startup(text: &str) -> Option<(&str, &str)> {
    text.lines().rev().find_map(|line| {
        let (_, loading) = line.split_once("Loading Minecraft ")?;
        let (_, loader) = loading.split_once(" with Fabric Loader ")?;
        Some((line, loader.split_whitespace().next()?))
    })
}

fn fabric_mixinextras_cast(text: &str) -> Option<(Diagnosis, String)> {
    let (_, version) = fabric_loader_startup(text)?;
    // MixinExtras read a redirect's list of targets as a single annotation.
    // https://github.com/LlamaLad7/MixinExtras/commit/78b31b1f
    static CAST: OnceLock<Regex> = OnceLock::new();
    let cast = CAST.get_or_init(|| {
        Regex::new(r"java\.lang\.ClassCastException: (?:class )?java\.util\.ArrayList cannot be cast to (?:class )?org\.objectweb\.asm\.tree\.AnnotationNode(?:[\s(]|$)").unwrap()
    });
    let tail = tail_of(text, MAX_TEXT);
    let exception = cast.find_iter(tail).find_map(|matched| {
        let mut lines = tail[matched.start()..].lines();
        let exception = lines.next()?;
        lines
            .map(|line| {
                line.strip_prefix(VANILLA_CRASH_TRACE)
                    .unwrap_or(line)
                    .trim_start()
            })
            .map_while(|line| line.strip_prefix("at "))
            .filter_map(|frame| frame.split_once('('))
            .any(|(method, _)| {
                let method = method.rsplit_once('/').map_or(method, |(_, name)| name);
                method.starts_with("com.llamalad7.mixinextras.")
            })
            .then_some(exception)
    })?;
    Some((
        Diagnosis {
            fix: Some(Fix::UpdateLoader),
            ..Diagnosis::new(
                "MixinExtras crashed on Fabric",
                format!(
                    "Try updating Fabric Loader {version}. If the crash continues, check for mod updates too."
                ),
                Vec::new(),
            )
        },
        head_of(exception, 2048).to_owned(),
    ))
}

fn diagnose_match(text: &str, java: Option<&str>) -> Option<(Diagnosis, String)> {
    diagnose_crash_match(text, java).or_else(|| hang_diagnosis(text, None))
}

fn diagnose_crash_match(text: &str, java: Option<&str>) -> Option<(Diagnosis, String)> {
    if let Some(disk_full) = disk_full_diagnosis(text) {
        return Some(disk_full);
    }
    if let Some(caller) = logged_vanilla_crash_save_failure(text) {
        let captured = captured_vanilla_crash(text);
        let (cause, matched) = captured
            .as_deref()
            .and_then(|trace| diagnose_rules(trace, java, None, &[]))
            .map_or_else(
                || {
                    (
                        None,
                        head_of(vanilla_crash_trace(text).next().unwrap_or(caller), 2048)
                            .to_owned(),
                    )
                },
                |(cause, matched)| (Some(cause), matched),
            );
        return Some((vanilla_crash_diagnosis(text, cause), matched));
    }
    diagnose_rules(text, java, None, &[])
}

fn launchwrapper_java_mismatch(text: &str) -> Option<(Diagnosis, String)> {
    const EXCEPTION: &str = "java.lang.ClassCastException: ";
    const CAST: &str = "jdk.internal.loader.ClassLoaders$AppClassLoader cannot be cast to ";
    const TARGET: &str = "java.net.URLClassLoader";
    let mut lines = text.lines().peekable();
    let error = loop {
        let line = lines.next()?;
        if let Some((prefix, message)) = line.split_once(EXCEPTION)
            && let Some(target) = message
                .trim_start_matches("class ")
                .trim_start_matches("java.base/")
                .strip_prefix(CAST)
            && target
                .trim_start_matches("class ")
                .trim_start_matches("java.base/")
                .split_whitespace()
                .next()
                == Some(TARGET)
            && lines.peek().is_some_and(|frame| {
                frame
                    .trim_start()
                    .starts_with("at net.minecraft.launchwrapper.Launch.")
            })
        {
            break &line[prefix.len()..];
        }
    };
    let major = text
        .lines()
        .rev()
        .find_map(crate::launch::process::diagnosed_runtime_major)
        .filter(|&major| major >= 9);
    let version = major.map_or_else(|| "9 or newer".to_owned(), |major| major.to_string());
    Some((
        Diagnosis::new(
            format!("{JAVA_8_MISMATCH_TITLE} {version}"),
            CUSTOM_JAVA_FIX,
            Vec::new(),
        ),
        head_of(error, 2048).to_owned(),
    ))
}

fn damaged_jar_diagnosis(text: &str, mods_dir: Option<&Path>) -> Option<(Diagnosis, String)> {
    static READING_JAR: OnceLock<Regex> = OnceLock::new();
    static BROKEN_ZIP: OnceLock<Regex> = OnceLock::new();
    let reading_jar = READING_JAR.get_or_init(|| {
        Regex::new(r#"(?i)\berror reading[ \t]+["']?(.+?\.jar)(?:["']?[ \t]*$|["']?[:!)])"#)
            .unwrap()
    });
    let broken_zip = BROKEN_ZIP.get_or_init(|| {
        Regex::new(
            r"(?i)\bZipException:[ \t]+(?:zip END header not found|invalid LOC header|zip file is empty)\b",
        )
        .unwrap()
    });
    let mut suspect = None;
    for line in text.lines().map(str::trim_start) {
        let broken = broken_zip.is_match(line);
        if let Some(reading) = reading_jar.captures(line) {
            suspect = reading.get(1).map(|path| path.as_str());
        } else if !broken
            && !["at ", "... ", "Caused by: "]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        {
            suspect = None;
        }
        if broken
            && let Some(path) = suspect
            && let Some((file, repair)) = damaged_jar_location(path, mods_dir)
        {
            let action = if repair {
                "Arctic will re-download it on the next launch."
            } else {
                "Re-download this mod and replace the damaged file."
            };
            return Some((
                Diagnosis {
                    repair,
                    ..Diagnosis::new(
                        "A game file was damaged",
                        format!("Minecraft couldn't read {file}. {action}"),
                        Vec::new(),
                    )
                },
                head_of(line, 2048).to_owned(),
            ));
        }
    }
    None
}

fn damaged_jar_location(path: &str, mods_dir: Option<&Path>) -> Option<(String, bool)> {
    // Logs can come from a different OS and can have their home directory redacted.
    let normalized = path.replace('\\', "/");
    let (parent, file) = normalized.rsplit_once('/')?;
    let folders: Vec<_> = parent.split('/').collect();
    if folders.contains(&"..") {
        return None;
    }
    let repair = folders.windows(2).any(|pair| {
        pair[0].eq_ignore_ascii_case("shared")
            && (pair[1].eq_ignore_ascii_case("libraries")
                || pair[1].eq_ignore_ascii_case("versions"))
    });
    let player_mod = folders.windows(4).any(|part| {
        part[0].eq_ignore_ascii_case("instances")
            && part[2].eq_ignore_ascii_case("minecraft")
            && part[3].eq_ignore_ascii_case("mods")
    }) || mods_dir.is_some_and(|dir| {
        let dir = dir.to_string_lossy().replace('\\', "/");
        normalized
            .to_ascii_lowercase()
            .strip_prefix(&dir.to_ascii_lowercase())
            .is_some_and(|suffix| suffix.starts_with('/'))
    });
    (repair || player_mod).then(|| (file.to_owned(), repair))
}

fn diagnose_rules(
    text: &str,
    java: Option<&str>,
    mods_dir: Option<&Path>,
    mods: &[crate::crash_conflicts::InstalledMod],
) -> Option<(Diagnosis, String)> {
    if let Some(fatal) = java_fatal_diagnosis(text) {
        return Some(fatal);
    }
    if let Some(runtime) = runtime_diagnosis(text) {
        return Some(runtime);
    }
    if let Some(native) = native_diagnosis(text) {
        return Some(native);
    }
    if let Some(dependency) = forge_dependency_diagnosis(text) {
        return Some(dependency);
    }
    if let Some(damaged) = damaged_jar_diagnosis(tail_of(text, MAX_TEXT), mods_dir) {
        return Some(damaged);
    }
    if let Some(mismatch) = launchwrapper_java_mismatch(tail_of(text, MAX_TEXT)) {
        return Some(mismatch);
    }
    if let Some((diagnosis, error)) = crate::crash_suspect::diagnose_threading(text, mods_dir, mods)
    {
        return Some((diagnosis, head_of(error, 2048).to_owned()));
    }
    if let Some((diagnosis, error)) =
        crate::crash_suspect::diagnose_unassigned_entity(text, mods_dir, mods)
    {
        return Some((diagnosis, error.to_owned()));
    }
    if let Some(driver) = sodium_driver_diagnosis(text) {
        return Some(driver);
    }
    if let Some(mixinextras) = fabric_mixinextras_cast(text) {
        return Some(mixinextras);
    }
    let text = tail_of(text, MAX_TEXT);

    if let Some(quilt) = quilt_load_error(text) {
        return Some(quilt);
    }

    if let Some(registry) = registry_sync_diagnosis(text) {
        return Some(registry);
    }

    static ONECONFIG_FAILURE: OnceLock<Regex> = OnceLock::new();
    let oneconfig_failure = ONECONFIG_FAILURE.get_or_init(|| Regex::new(r"(?im)^.*(?:OneConfig[^\r\n]*failed to download|(?:failed to download)[^\r\n]*(?:OneConfig|polyfrost)).*$").unwrap());
    if let Some(matched) = oneconfig_failure.find(text) {
        return Some((
            Diagnosis::new(
                "Patcher couldn't download OneConfig (network)",
                "Patcher needs OneConfig from Polyfrost's servers. Arctic will retry preparing it on the next launch and skip its auto-installed Patcher if the download fails. Check your connection, or turn Patcher off in instance settings.",
                vec!["Patcher".into(), "OneConfig".into()],
            ),
            head_of(matched.as_str(), 2048).to_owned(),
        ));
    }

    if let Some(wrong_game) = fabric_wrong_game(text) {
        return Some(wrong_game);
    }

    if let Some((config, evidence)) = crate::crash_conflicts::diagnose_config(text, mods) {
        return Some((config, head_of(evidence, 2048).to_owned()));
    }

    diagnose_log_rules(text, java, mods_dir)
}

fn diagnose_log_rules(
    text: &str,
    java: Option<&str>,
    mods_dir: Option<&Path>,
) -> Option<(Diagnosis, String)> {
    let secondary_report = crash_exception(text).is_some_and(secondary_stack);
    for (rule, re) in RULES.iter().zip(compiled()) {
        for c in re.captures_iter(text) {
            let matched = c.get(0)?;
            let start = text[..matched.start()].rfind('\n').map_or(0, |n| n + 1);
            let end = text[matched.start()..]
                .find('\n')
                .map_or(text.len(), |n| matched.start() + n);
            let line = &text[start..end];
            if class_loading_warning(line)
                || secondary_mod_failure(line)
                || (secondary_report && line.trim_start().starts_with("Suspected Mod"))
            {
                continue;
            }
            if matches!(
                rule.pattern,
                crate::crash_conflicts::FABRIC_MIXIN_APPLY_FAILURE
                    | crate::crash_conflicts::MIXIN_APPLY_FAILURE
            ) && crate::crash_conflicts::mixin_warning(line)
            {
                continue;
            }
            if (rule.pattern.contains("NoSuchMethodError")
                || rule.pattern.contains("ClassNotFoundException"))
                // The JVM's missing entry point is fatal without an exception block.
                && !rule.pattern.contains("Could not find or load main class")
                && !fatal_linkage(text, matched.start())
            {
                continue;
            }
            let context = if rule.pattern == OUT_OF_MEMORY {
                java.unwrap_or(text)
            } else {
                text
            };
            let diagnosis = if rule.pattern == LINKAGE_ERROR
                && let Some(diagnosis) = missing_entrypoint_library(text, matched.start(), mods_dir)
            {
                diagnosis
            } else {
                (rule.explain)(&c, context)
            };
            if !diagnosis.title.is_empty() {
                // The mixin rules select the actual failure, which can differ from their first match.
                if matches!(
                    rule.pattern,
                    crate::crash_conflicts::MIXIN_CONFIG_PATTERN
                        | crate::crash_conflicts::EXPLICIT_MIXIN_FAILURE_PATTERN
                ) {
                    let line = crate::crash_conflicts::mixin_failure_line(text)?;
                    return Some((diagnosis, head_of(line, 2048).to_owned()));
                }
                return Some((diagnosis, head_of(line, 2048).to_owned()));
            }
        }
    }
    None
}

/// The exception must name a native library in a managed JDK layout. Other
/// UnsatisfiedLinkErrors are commonly caused by mods, LWJGL or system libraries.
pub(crate) fn runtime_native_error(line: &str) -> Option<regex::Match<'_>> {
    if !(line.contains("UnsatisfiedLinkError")
        || line.contains("The specified procedure could not be found")
        || line.contains("Can't find dependent libraries"))
    {
        return None;
    }
    const COMPONENT: &str = r"(?:java-runtime-[\w.-]+|jre-legacy(?:-x64)?)";
    static LIBRARY: OnceLock<Regex> = OnceLock::new();
    let library = LIBRARY.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)(?P<runtime>[/\\]runtimes[/\\](?:{COMPONENT}|\.named[/\\]{COMPONENT}[/\\]Arctic Client))[/\\]{}",
            r#"(?:jre\.bundle[/\\]Contents[/\\]Home[/\\])?(?:jre[/\\])?(?:bin|lib)[/\\](?:[\w-]+[/\\])*[\w.-]+\.(?:dll|dylib|so(?:\.\d+)*)(?:$|[:\s)'"])"#,
        )).unwrap()
    });
    library.captures(line)?.name("runtime")
}

pub(crate) fn runtime_diagnosis(text: &str) -> Option<(Diagnosis, String)> {
    let line = text
        .lines()
        .find(|line| runtime_native_error(line).is_some())?;
    Some((
        Diagnosis::new(
            "The Java runtime is broken",
            "Java could not load one of its own libraries. Arctic will verify and repair the runtime on the next launch.",
            Vec::new(),
        ),
        head_of(line.trim(), 2048).to_owned(),
    ))
}

const SODIUM_DRIVER_TITLE: &str = "Sodium refused to start on this graphics driver";

fn sodium_driver_diagnosis(text: &str) -> Option<(Diagnosis, String)> {
    fn sodium_call(frame: &str) -> Option<&str> {
        let call = if frame.contains('(') {
            freeze_frame(frame)?
        } else {
            frame.rsplit('/').next()?
        };
        call.strip_prefix("net.caffeinemc.mods.sodium.client.")
            .or_else(|| call.strip_prefix("me.jellysquid.mods.sodium.client."))
    }
    let is_driver_check = |frame: &&str| {
        sodium_call(frame)
            .is_some_and(|call| call.starts_with("compatibility.checks.GraphicsDriverChecks."))
    };
    let exit_frame = text
        .lines()
        .filter_map(|line| {
            let entry = line.strip_prefix("[Arctic diagnostics] ")?;
            entry
                .strip_prefix("exit caller other ")
                .or_else(|| entry.trim_start().strip_prefix("at "))
        })
        .find(is_driver_check);
    let frame = exit_frame.or_else(|| {
        if !freeze_evidence(text) {
            return None;
        }
        let hang_frame = text
            .rsplit_once("[Arctic diagnostics] hang begin")
            .filter(|(_, dump)| !dump.contains("[Arctic diagnostics] hang recovered"))
            .and_then(|(_, dump)| {
                dump.lines()
                    .skip_while(|line| !line.starts_with("[Arctic diagnostics] thread \""))
                    .skip(1)
                    .map_while(|line| line.trim_start().strip_prefix("at "))
                    .find(is_driver_check)
            });
        hang_frame.or_else(|| {
            if !text.contains("---- logs/arctic-freezes/") {
                return None;
            }
            let snapshot = latest_freeze_snapshot(text);
            if !snapshot.contains("(still frozen when written)") {
                return None;
            }
            freeze_hot_frames(snapshot)
                .map(|(_, frame, _)| frame)
                .chain(
                    snapshot
                        .lines()
                        .filter_map(|line| line.trim_start().strip_prefix("at ")),
                )
                .find(is_driver_check)
        })
    })?;
    let mut detail = "The GPU or its driver is unsupported or too old for Sodium.".to_owned();
    if let Some(inventory) = text.lines().rev().find_map(|line| {
        let (_, adapters) = line
            .strip_prefix("[Arctic diagnostics] runtime:")?
            .split_once("display adapters=")?;
        let (_, entries) = adapters.split_once(" [")?;
        let (inventory, _) = entries.rsplit_once(']')?;
        (!inventory.is_empty()).then_some(inventory)
    }) {
        let mut adapters = Vec::new();
        // The date may contain a comma. The adapter entries end with a closing parenthesis.
        for entry in inventory.split_inclusive("), ") {
            let entry = entry.trim_end_matches(", ");
            if !adapters.contains(&entry) {
                adapters.push(entry);
            }
        }
        detail.push_str(&format!(
            " Available display adapters include {}.",
            adapters.join(", ")
        ));
    }
    detail.push_str(
        " Update your graphics driver. If your computer has a dedicated GPU, make sure Minecraft uses it.",
    );
    Some((
        Diagnosis::new(SODIUM_DRIVER_TITLE, detail, vec!["sodium".into()]),
        head_of(frame, 2048).to_owned(),
    ))
}

pub(crate) fn latest_freeze_snapshot(text: &str) -> &str {
    text.rsplit_once("---- logs/arctic-freezes/")
        .map_or(text, |(_, snapshot)| {
            snapshot.split("\n---- ").next().unwrap_or(snapshot)
        })
}

pub(crate) fn freeze_hot_frames(snapshot: &str) -> impl Iterator<Item = (u8, &str, &str)> {
    const HEADING: &str = "== Where the time went (frames present in the most samples) ==";
    snapshot
        .rsplit_once(HEADING)
        .into_iter()
        .flat_map(|(_, rows)| {
            rows.lines()
                .skip(1)
                .take_while(|line| !line.trim().is_empty() && !line.starts_with("== "))
        })
        .filter_map(|line| {
            let (percent, frame) = line.trim().split_once('%')?;
            Some((percent.parse().ok()?, frame.trim(), line.trim()))
        })
}

fn freeze_frame(entry: &str) -> Option<&str> {
    let (call, location) = entry.split_once('(')?;
    location.split_once(')')?;
    // Hidden lambda classes can carry a process-specific address after a slash.
    if call.contains("$$Lambda") || call.contains("/0x") {
        return None;
    }
    let call = call.rsplit('/').next()?;
    let (class, _) = call.rsplit_once('.')?;
    if ["java.", "javax.", "jdk.", "sun.", "com.sun."]
        .iter()
        .any(|prefix| class.starts_with(prefix))
        || matches!(
            class,
            "net.minecraft.class_1255"
                | "net.minecraft.util.thread.ThreadExecutor"
                | "net.minecraft.util.thread.BlockableEventLoop"
        )
        || !call
            .chars()
            .all(|c| c.is_alphanumeric() || "._$<>".contains(c))
    {
        return None;
    }
    Some(call)
}

fn freeze_group_frame<'a>(text: &'a str, snapshot: &'a str) -> Option<&'a str> {
    let top = snapshot.lines().find_map(|line| {
        line.strip_prefix("Arctic: game froze for ")?
            .split_once(", mostly in ")
            .map(|(_, frame)| frame)
    });
    let hot = freeze_hot_frames(snapshot).map(|(_, frame, _)| frame);
    let stack = snapshot
        .split_once("== Distinct client-thread stacks")
        .into_iter()
        .flat_map(|(_, stacks)| {
            stacks
                .lines()
                .skip_while(|line| !line.starts_with('[') && !line.starts_with("== "))
                .skip(1)
                .map_while(|line| line.trim_start().strip_prefix("at "))
        });
    // The long-hang dump writes the client thread first, followed by the other threads.
    let dump = text
        .rsplit_once("[Arctic diagnostics] hang begin")
        .into_iter()
        .flat_map(|(_, dump)| {
            dump.lines()
                .skip_while(|line| !line.starts_with("[Arctic diagnostics] thread \""))
                .skip(1)
                .map_while(|line| line.trim_start().strip_prefix("at "))
        });
    top.into_iter()
        .chain(hot)
        .chain(stack)
        .chain(dump)
        .find_map(freeze_frame)
}

fn iris_shader_loading_frame(snapshot: &str) -> Option<&str> {
    let hot = freeze_hot_frames(snapshot).map(|(_, frame, _)| frame);
    let stacks = snapshot
        .split_once("== Distinct client-thread stacks")
        .into_iter()
        .flat_map(|(_, stacks)| {
            stacks
                .lines()
                .skip(1)
                .take_while(|line| !line.starts_with("== "))
                .filter_map(|line| line.trim_start().strip_prefix("at "))
        });
    hot.chain(stacks).find(|frame| {
        freeze_frame(frame)
            .and_then(|call| {
                call.strip_prefix("net.irisshaders.iris.")
                    .or_else(|| call.strip_prefix("net.coderbot.iris."))
            })
            .is_some_and(|call| {
                matches!(
                    call,
                    "Iris.reload"
                        | "Iris.loadShaderpack"
                        | "Iris.loadExternalShaderpack"
                        | "shaderpack.ShaderPack.<init>"
                )
            })
    })
}

const JEI_RECIPE_LOADING_TITLE: &str = "JEI was still loading recipes";

fn jei_recipe_loading_frame<'a>(text: &'a str, snapshot: &'a str) -> Option<&'a str> {
    fn loading(frame: &str) -> bool {
        let Some((class, method)) = freeze_frame(frame)
            .and_then(|call| call.strip_prefix("mezz.jei."))
            .and_then(|call| call.rsplit_once('.'))
        else {
            return false;
        };
        let class = class.rsplit('.').next().unwrap_or(class);
        class.ends_with("RecipeMaker")
            || matches!(
                (class, method),
                ("JeiStarter", "start")
                    | ("PluginLoader", "createRecipeManager")
                    | ("ClientLifecycleHandler", "startJei")
            )
    }

    if let Some((_, dump)) = text.rsplit_once("[Arctic diagnostics] hang begin") {
        // The watchdog puts the client thread first in the dump.
        let mut lines = dump.lines().skip(1);
        let thread = lines.next()?.strip_prefix("[Arctic diagnostics] thread ")?;
        if !matches!(
            thread,
            "\"Render thread\" RUNNABLE" | "\"Client thread\" RUNNABLE"
        ) || dump.contains("[Arctic diagnostics] hang recovered")
        {
            return None;
        }
        return lines
            .map_while(|line| line.trim_start().strip_prefix("at "))
            .find(|frame| loading(frame));
    }
    if !snapshot.contains("(still frozen when written)") {
        return None;
    }
    freeze_hot_frames(snapshot)
        .find(|(percent, frame, _)| (51..=100).contains(percent) && loading(frame))
        .map(|(_, frame, _)| frame)
}

const GRAPHICS_DRIVER_HANG_TITLE: &str = "The graphics driver stopped responding";

fn graphics_wait(frame: &str) -> bool {
    let Some((class, method)) = freeze_frame(frame).and_then(|call| call.rsplit_once('.')) else {
        return false;
    };
    if matches!(
        (class, method),
        ("com.mojang.blaze3d.opengl.GlFence", "awaitCompletion")
            | (
                "com.mojang.blaze3d.opengl.GlStateManager",
                "_glClientWaitSync"
            )
    ) {
        return true;
    }
    let method = method.strip_prefix('n').unwrap_or(method);
    frame.ends_with("(Native Method)")
        && ((class.starts_with("org.lwjgl.opengl.")
            && matches!(
                method,
                "glClientWaitSync"
                    | "glWaitSync"
                    | "glFinish"
                    | "glClientWaitSyncAPPLE"
                    | "glWaitSyncAPPLE"
                    | "wglSwapBuffers"
                    | "glXSwapBuffers"
                    | "SwapBuffers"
            ))
            || (class == "org.lwjgl.glfw.GLFW" && method == "glfwSwapBuffers")
            || (class == "org.lwjgl.egl.EGL10" && method == "eglSwapBuffers"))
}

fn graphics_driver_hang_frame<'a>(snapshot: &'a str, summary: &'a str) -> Option<&'a str> {
    if let Some(frame) = summary
        .strip_prefix("Hang at exit: client hung for at least ")
        .and_then(|line| line.split_once(" in "))
        .and_then(|(_, frame)| frame.split_once(';'))
        .map(|(frame, _)| frame)
        && summary.contains("no recovery recorded")
        && graphics_wait(frame)
    {
        return Some(frame);
    }
    // A completed snapshot can predate a later Windows hang termination.
    if snapshot.lines().any(|line| {
        line.starts_with("duration: ") && !line.ends_with(" (still frozen when written)")
    }) {
        return None;
    }
    freeze_hot_frames(snapshot)
        .find(|(percent, frame, _)| (51..=100).contains(percent) && graphics_wait(frame))
        .map(|(_, _, line)| line)
}

fn graphics_driver_hang_detail(text: &str) -> String {
    use chrono::{Months, NaiveDate, Utc};

    let mut detail = "The graphics driver hung while Minecraft was drawing.".to_owned();
    let adapters = text.lines().rev().find_map(|line| {
        let (_, runtime) = line.split_once("[Arctic diagnostics] runtime: ")?;
        let (_, adapters) = runtime.split_once("display adapters=")?;
        let (_, adapters) = adapters.split_once('[')?;
        adapters.split_once(']').map(|(list, _)| list)
    });
    let mut old_drivers = false;
    if let Some(adapters) = adapters {
        static ADAPTER: OnceLock<Regex> = OnceLock::new();
        let adapter = ADAPTER.get_or_init(|| {
            Regex::new(r"([^,]+?)(?: \(driver [^,)]*(?:, ([^)]*))?\))?(?:, |$)").unwrap()
        });
        let cutoff = Utc::now().date_naive().checked_sub_months(Months::new(60));
        let mut names = Vec::new();
        // Windows reports display devices without identifying the adapter used by the game.
        old_drivers = true;
        for entry in adapter.captures_iter(adapters) {
            let name = entry[1].trim();
            let date = entry.get(2).map(|date| date.as_str());
            let parsed_date =
                date.and_then(|date| NaiveDate::parse_from_str(date, "%m-%d-%Y").ok());
            old_drivers &= parsed_date
                .zip(cutoff)
                .is_some_and(|(date, cutoff)| date < cutoff);
            let name = match (parsed_date, date) {
                (Some(date), _) => format!("{name} (driver from {date})"),
                (None, Some(date)) => format!("{name} (driver date {date})"),
                (None, None) => name.to_owned(),
            };
            if !names.contains(&name) {
                names.push(name);
            }
        }
        old_drivers &= !names.is_empty();
        if !names.is_empty() {
            detail.push_str(&format!(" The launcher detected {}.", names.join(", ")));
        }
    }
    detail.push_str(" Update your graphics driver.");
    if old_drivers {
        detail.push_str(" Newer Minecraft versions may run poorly on this hardware. Try an older Minecraft version or close other GPU-heavy apps before playing.");
    }
    detail
}

fn hang_diagnosis(full_text: &str, mods_dir: Option<&Path>) -> Option<(Diagnosis, String)> {
    let text = tail_of(full_text, MAX_TEXT);
    if !freeze_evidence(text)
        || text.contains("---- Minecraft Crash Report ----")
        || text.contains("---- crash report ----\n")
        || text.contains("A fatal error has been detected by the Java Runtime Environment")
    {
        return None;
    }
    // A fatal exception takes precedence even if none of the specific rules knows its cause.
    let exceptions = Regex::new(r"(?:[\w]+\.)+[\w$]+(?:Exception|Error)").unwrap();
    if exceptions
        .find_iter(text)
        .any(|m| fatal_linkage(text, m.start()))
    {
        return None;
    }
    let summary = text
        .lines()
        .rev()
        .find(|l| l.contains("Hang at exit:"))
        .or_else(|| {
            text.lines().rev().find(|l| {
                l.contains("(still frozen when written)")
                    || l.contains("[Arctic diagnostics] hang begin")
                    || l.contains("0xCFFFFFFF")
            })
        })
        .unwrap_or("The run contains an unrecovered watchdog freeze.");
    let snapshot = latest_freeze_snapshot(text);
    if let Some(frame) = jei_recipe_loading_frame(text, snapshot) {
        return Some((
            Diagnosis::new(
                JEI_RECIPE_LOADING_TITLE,
                "JEI builds its recipe list when you join a world. This can take several minutes in large modpacks. Give it more time to finish, or remove or replace mods that add very large recipe sets.",
                vec!["jei".into()],
            ),
            format!("{summary}\n{frame}"),
        ));
    }
    if !snapshot
        .lines()
        .any(|line| line.starts_with("duration: ") && !line.contains("(still frozen when written)"))
        && let Some(frame) = iris_shader_loading_frame(snapshot)
    {
        return Some((
            Diagnosis::new(
                IRIS_SHADER_LOADING_TITLE,
                IRIS_SHADER_LOADING_DETAIL,
                Vec::new(),
            ),
            format!("{summary}\n{frame}"),
        ));
    }
    if let Some(frame) = graphics_driver_hang_frame(snapshot, summary) {
        let matched = if summary.contains(frame) {
            summary.to_owned()
        } else {
            format!("{summary}\n{frame}")
        };
        return Some((
            Diagnosis::new(
                GRAPHICS_DRIVER_HANG_TITLE,
                graphics_driver_hang_detail(full_text),
                Vec::new(),
            ),
            matched,
        ));
    }
    let short_menu_freeze = snapshot.contains("not in a world (menu/loading)")
        || snapshot.contains("no world (menu/loading screen)");
    // A first snapshot at about three seconds gives little evidence about who caused a menu stall.
    let short_menu_freeze = short_menu_freeze
        && snapshot.lines().any(|line| {
            line.strip_prefix("duration: ")
                .and_then(|duration| duration.split_once('s'))
                .is_some_and(|(seconds, _)| seconds.parse::<f32>().is_ok_and(|s| s <= 4.0))
        });
    if !short_menu_freeze && let Some(culprit) = crate::crash_suspect::hang_culprit(text, mods_dir)
    {
        let owner = if culprit.mod_info.ours {
            "This is an Arctic bug."
        } else {
            "The samples point to third-party code."
        };
        let method = format!("{}.{}", culprit.class, culprit.method);
        return Some((
            Diagnosis::new(
                format!("{} froze the game", culprit.mod_info.name),
                format!(
                    "{summary}\nThe watchdog found {method} in {}% of client-thread samples. {owner}",
                    culprit.percent
                ),
                vec![culprit.mod_info.id],
            ),
            format!("{summary}\n{}", culprit.sample),
        ));
    }
    let heap =
        Regex::new(r"heap: (\d+) MB used of (\d+) MB max(?: at start, (\d+) MB at end)?").unwrap();
    let gc = Regex::new(r"gc during freeze: (\d+) collections, (\d+) ms").unwrap();
    let memory = heap
        .captures_iter(text)
        .find(|c| {
            let used = c[1]
                .parse::<u64>()
                .unwrap_or(0)
                .max(c.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(0));
            let max = c[2].parse::<u64>().unwrap_or(0);
            max > 0 && used * 100 >= max * 90
        })
        .or_else(|| {
            gc.captures_iter(text).find(|c| {
                c[1].parse::<u64>().unwrap_or(0) >= 10 && c[2].parse::<u64>().unwrap_or(0) >= 1000
            })
        });
    if let Some(memory) = memory {
        let cap = if text.contains("is more than this computer can spare: using") {
            " Arctic capped the memory because this computer could not spare the requested amount."
        } else {
            ""
        };
        Some((
            Diagnosis::new(
                "Minecraft ran out of memory and froze",
                format!(
                    "The game froze with memory pressure. Give it more memory if the PC has enough available; otherwise use fewer or lighter mods.{cap}"
                ),
                Vec::new(),
            ),
            format!("{summary}\nmatched: {}", &memory[0]),
        ))
    } else {
        let create_world = short_menu_freeze
            && snapshot.contains("(still frozen when written)")
            && freeze_hot_frames(snapshot).any(|(percent, frame, _)| {
                percent >= 60
                    && freeze_frame(frame).is_some_and(|call| {
                        call.starts_with("net.minecraft.class_525.")
                            || call.contains(".CreateWorldScreen.")
                    })
            });
        let title = if create_world {
            "The game was closed while the Create World screen was loading"
        } else {
            "The game froze and was closed"
        };
        Some((
            Diagnosis::new(title, summary, Vec::new()),
            summary.to_owned(),
        ))
    }
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
    /// Installed and currently free physical memory in MiB, if they could be read.
    pub ram_total_mb: Option<u64>,
    pub ram_free_mb: Option<u64>,
    /// How the player can be reached (their own words, optional); filled in by the app when it sends.
    pub contact: String,
}

/// Information from the process watcher and the launch plan.
#[derive(Default)]
pub struct ReportRun<'a> {
    pub java: Option<&'a str>,
    pub diagnostics: Option<&'a ExitDiagnostics>,
}

/// Lines of the game log's end that are sent.
const REPORT_LOG_LINES: usize = 150;
/// Leave room under the server's 48 KB cap.
const REPORT_BYTES: usize = 47 * 1024;
const REPORT_LOG_BYTES: usize = 8 * 1024;
/// Usual size of the crash attachment. Its stacks may borrow from the rest of the upload.
const REPORT_CRASH_BYTES: usize = 16 * 1024;
/// Maximum crash excerpt, leaving at least 19 KiB for priority evidence and logs.
const REPORT_CRASH_CORE_BYTES: usize = 28 * 1024;
/// Latest registry warnings/errors, reserved ahead of verbose log tails and crash inventories.
const REPORT_REGISTRY_BYTES: usize = 6 * 1024;
/// How Java was started: its flags and the classpath's file names.
const REPORT_JAVA_BYTES: usize = 5 * 1024;
/// The launcher's own log (sign-in failures, downloads, launch steps): its last lines.
const REPORT_LAUNCHER_LINES: usize = 80;
const REPORT_LAUNCHER_BYTES: usize = 6 * 1024;

fn native_first_error(text: &str) -> Option<Cow<'_, str>> {
    let (report, fatal) = if let Some(fatal) = java_fatal_error(text) {
        (fatal.0, Some(fatal.1))
    } else {
        let fatal = text.lines().find(|line| {
            line.trim_start_matches(['#', ' '])
                .starts_with("EXCEPTION_ACCESS_VIOLATION")
        });
        (text, fatal)
    };
    let frame = crate::stability::problematic_frame(report);
    let error = match fatal {
        // The code is stable across runs. Fault addresses and process/thread IDs vary.
        Some(fatal) => fatal.split_once(" at pc=").map_or(fatal, |(kind, _)| kind),
        None => {
            frame?;
            "Error: Native crash"
        }
    };
    let library = frame
        .and_then(|frame| frame.strip_prefix("C "))
        .and_then(|frame| frame.trim_start().strip_prefix('['))
        .and_then(|frame| frame.split_once(']'))
        .map(|(library, _)| library.split_once("+0x").map_or(library, |(name, _)| name))
        .and_then(|library| library.rsplit(['/', '\\']).next());

    // HotSpot lists Java methods in the mixed native stack and sometimes in a separate stack.
    static JAVA_FRAME: OnceLock<Regex> = OnceLock::new();
    let java_frame =
        JAVA_FRAME.get_or_init(|| Regex::new(r"^(?:j\s+|J\s+\d+\s+\S+\s+)([\w.$/]+)\(").unwrap());
    let caller = ["Native frames:", "Java frames:"]
        .into_iter()
        .flat_map(|header| {
            report
                .lines()
                .skip_while(move |line| !line.starts_with(header))
                .skip(1)
                .take_while(|line| !line.trim().is_empty() && !exception_record_boundary(line))
        })
        .filter_map(|line| java_frame.captures(line))
        .map(|captures| captures.get(1).unwrap().as_str())
        .find(|method| {
            ![
                "java",
                "javax",
                "jdk",
                "sun",
                "org.lwjgl",
                "org/lwjgl",
                "net.minecraft",
                "net/minecraft",
                "com.mojang",
                "com/mojang",
            ]
            .iter()
            .any(|prefix| {
                method
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with(['.', '/']))
            })
        });

    if library.is_none() && caller.is_none() {
        return Some(error.into());
    }
    let mut summary = error.to_owned();
    if let Some(library) = library {
        summary.push_str(" in ");
        summary.push_str(library);
    }
    if let Some(caller) = caller {
        summary.push_str("\nJava frame: ");
        summary.push_str(caller);
    }
    Some(summary.into())
}

/// Prefer the captured exception or a crash document. In ordinary logs, require an exception
/// followed by a Java stack frame in the same log record.
fn first_java_error<'a>(log: &'a str, crash_report: Option<&'a str>) -> Option<&'a str> {
    if let Some(stack) = crash_report
        .and_then(crash_exception)
        .or_else(|| crash_exception(log))
    {
        // Java indents causes belonging to suppressed failures. The main chain's causes are bare.
        return stack
            .lines()
            .rev()
            .find(|line| line.starts_with("Caused by: "))
            .or_else(|| stack.lines().next());
    }
    if let Some(error) = vanilla_crash_trace(log).next() {
        return Some(error);
    }
    if let Some(error) = crash_report
        .and_then(|report| quilt_load_errors().find(report))
        .or_else(|| quilt_load_errors().find(log))
    {
        return Some(error.as_str());
    }
    static EXCEPTION: OnceLock<Regex> = OnceLock::new();
    let exception = EXCEPTION
        .get_or_init(|| Regex::new(r"(?:[\w$]+\.)*[\w$]*(?:Exception|Error)(?::|\s*$)").unwrap());
    let select = |text: &'a str, report: bool| {
        let mut warning = false;
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            if exception_record_boundary(line) {
                warning = line.contains("/WARN]")
                    || line.contains("/INFO]")
                    || background_auth_record(
                        line,
                        lines.clone().take_while(|l| !exception_record_boundary(l)),
                    );
            }
            // A log tail may start inside the background request's stack trace.
            if !warning
                && matches!(
                    line.trim_start().trim_start_matches("Caused by: "),
                    "com.mojang.authlib.exceptions.InvalidCredentialsException: Status: 401"
                        | "com.mojang.authlib.exceptions.MinecraftClientHttpException: Status: 401"
                )
                && lines
                    .clone()
                    .take_while(|line| !exception_record_boundary(line))
                    .any(|line| {
                        (line.contains("status=401,") && line.contains("path=/player/attributes,"))
                            || (line.trim_start().starts_with("at ")
                                && line.contains(".YggdrasilUserApiService.fetchProperties("))
                    })
            {
                warning = true;
            }
            if !warning
                && line
                    .trim_start()
                    .starts_with("com.mojang.realmsclient.exception.RealmsServiceException: ")
                && line.contains("Could not connect to Realms")
                && lines
                    .clone()
                    .take_while(|line| !exception_record_boundary(line))
                    .any(|line| {
                        line.trim_start().starts_with("at ")
                            && line.contains(".RealmsClient.fetchFeatureFlags(")
                    })
            {
                warning = true;
            }
            if services_key_failure(line) {
                warning = true;
            }
            if warning || line.contains("pack.mcmeta") || line.contains("pack metadata") {
                continue;
            }
            let Some(found) = exception.find(line) else {
                continue;
            };
            // A report's first bare exception is its main exception, before its cause chain.
            if report && found.start() == 0 {
                return Some(line.trim());
            }
            if !report
                && lines
                    .clone()
                    .take_while(|l| !exception_record_boundary(l))
                    .any(|l| l.trim_start().starts_with("at "))
            {
                return Some(line[found.start()..].trim());
            }
        }
        None
    };
    crash_report
        .filter(|report| !report.contains("---- Minecraft Crash Report ----"))
        .and_then(|report| select(report, true))
        .or_else(|| select(log, false))
}

impl Report {
    /// `lines` is the game's log, `launcher_lines` the launcher's own log, `crash_report` the file
    /// Minecraft wrote (if it did), `java` how Java was started (see `LaunchPlan::java_summary`).
    /// `title` is what Arctic made of it, or empty.
    pub fn build(
        game: &str,
        loader: &str,
        title: &str,
        lines: &[&str],
        launcher_lines: &[&str],
        crash_report: Option<&str>,
        java: Option<&str>,
    ) -> Self {
        Self::build_with_run(
            game,
            loader,
            title,
            lines,
            launcher_lines,
            crash_report,
            ReportRun {
                java,
                diagnostics: None,
            },
        )
    }

    pub fn build_with_run(
        game: &str,
        loader: &str,
        title: &str,
        lines: &[&str],
        launcher_lines: &[&str],
        crash_report: Option<&str>,
        run: ReportRun<'_>,
    ) -> Self {
        let java = run.java;
        let own = Own::this_pc();
        let all = lines.join("\n");
        // Reserve the resolved mod inventory and agent failure context before trimming verbose
        // logs/reports. Fabric's mod list often starts beyond the old 16 KB report prefix.
        let mods = compact_mod_inventory(&all, crash_report.unwrap_or(""));
        let mixin = mixin_failure_evidence(&all);
        let mut priority = String::new();
        let diagnosis_text = format!(
            "{all}\n{}\n{}\n{}",
            crash_report
                .map(|r| format!("---- crash report ----\n{r}"))
                .unwrap_or_default(),
            launcher_lines.join("\n"),
            java.unwrap_or("")
        );
        // Keep the exact triggering line even when it falls outside the uploaded log tail.
        let crash = diagnose_crash_match(&diagnosis_text, java);
        let diagnosis_tail = tail_of(&diagnosis_text, MAX_TEXT);
        let watchdog = if crash.is_none() {
            hang_diagnosis(&diagnosis_text, None)
        } else {
            None
        };
        let freeze_frame = if watchdog.is_some() && title != SAVE_FAILED_TITLE {
            freeze_group_frame(diagnosis_tail, latest_freeze_snapshot(diagnosis_tail))
        } else {
            None
        };
        let diagnosis = crash.or(watchdog);
        let failed_save = title == SAVE_FAILED_TITLE
            || diagnosis
                .as_ref()
                .is_some_and(|(d, _)| d.title == SAVE_FAILED_TITLE);
        if let Some(frame) = freeze_frame {
            // The server groups by the first Error: line or exception and strips digits before
            // hashing. Encode the frame with letters so intermediary class/method numbers survive.
            let signature: String = Sha256::digest(frame.as_bytes())[..8]
                .iter()
                .flat_map(|byte| {
                    [
                        char::from(b'a' + (byte >> 4)),
                        char::from(b'a' + (byte & 15)),
                    ]
                })
                .collect();
            priority.push_str(&format!("\nError: Watchdog freeze at exit [{signature}]\n"));
        }
        // A consequence message is kept in the original stack, while the summary uses the cause.
        let java_error = first_java_error(&all, crash_report);
        let sodium_driver = diagnosis
            .as_ref()
            .filter(|(d, _)| d.title == SODIUM_DRIVER_TITLE);
        let sodium_error =
            sodium_driver.map(|(_, frame)| format!("Error: {SODIUM_DRIVER_TITLE}\n{frame}"));
        let wrong_java = diagnosis
            .as_ref()
            .filter(|(d, _)| d.title.starts_with(JAVA_8_MISMATCH_TITLE));
        let native_error = crash_report
            .and_then(native_first_error)
            .or_else(|| native_first_error(&all));
        let error = if let Some(error) = native_error.as_deref().or(sodium_error.as_deref()) {
            Some(error)
        } else if let Some((_, matched)) = wrong_java {
            Some(matched.as_str())
        } else if java_error.is_some_and(secondary_mod_failure)
            || crash_exception(&diagnosis_text).is_some_and(secondary_stack)
        {
            diagnosis.as_ref().map(|(_, matched)| matched.as_str())
        } else {
            java_error
        };
        if let Some(error) = error {
            priority.push_str(&format!(
                "\n---- first error ----\n{}\n",
                scrub(head_of(error, 2048), &own)
            ));
        }
        if let Some((diagnosis, matched)) = &diagnosis {
            priority.push_str(&format!(
                "\n---- diagnosis ----\n{}\nmatched: {}\n",
                diagnosis.title,
                scrub(matched, &own)
            ));
            if (diagnosis.title == SAVE_FAILED_TITLE && vanilla_crash_trace(&all).next().is_some())
                || diagnosis.title == crate::crash_suspect::UNASSIGNED_ENTITY_TITLE
                || diagnosis.title == SODIUM_DRIVER_TITLE
                || diagnosis.title == GRAPHICS_DRIVER_HANG_TITLE
            {
                priority.push_str(&scrub(head_of(&diagnosis.detail, 2048), &own));
                priority.push('\n');
            }
        }
        #[cfg(windows)]
        {
            priority.push_str("\n---- Windows ----\n");
            priority.push_str(&scrub(&crate::system::windows::description(), &own));
            priority.push('\n');
        }
        if let Some(diagnostics) = run.diagnostics {
            if let Some(dll_report) = diagnostics.dll_report() {
                priority.push_str("\n---- missing DLL checks ----\n");
                priority.push_str(&scrub(dll_report, &own));
            }
            if diagnostics.output.count() != 0 {
                priority.push_str("\n---- child stdout/stderr ----\n");
                priority.push_str(&scrub(&diagnostics.output.report(), &own));
            }
        }
        let vanilla_evidence = all
            .lines()
            .filter(|line| line.starts_with(VANILLA_CRASH))
            .collect::<Vec<_>>()
            .join("\n");
        if !vanilla_evidence.is_empty() {
            priority.push_str("\n---- vanilla crash ----\n");
            priority.push_str(&scrub(head_of(&vanilla_evidence, 18 * 1024), &own));
            priority.push('\n');
        }
        let registry = registry_failure_evidence(&all);
        if !registry.is_empty() {
            priority.push_str("\n---- registry failure context ----\n");
            priority.push_str(&scrub(&registry, &own));
        }
        if !mods.is_empty() {
            priority.push_str(&mod_inventory_section(&mods));
        }
        if !mixin.is_empty() {
            priority.push_str("\n---- mixin failure context ----\n");
            priority.push_str(&scrub(&mixin, &own));
        }
        let early = first_exceptions(&all);
        if !early.is_empty() {
            priority.push_str("\n---- first exceptions ----\n");
            priority.push_str(&scrub(&early, &own));
        }
        let ram_total_mb = crate::system::total_memory_mb();
        let ram_free_mb = crate::system::free_memory_mb();
        priority.push_str(&memory_section(ram_total_mb, ram_free_mb));

        // Reserve the completed priority sections first, and leave room for logs when stacks
        // are large and priority evidence is small.
        let crash_budget = REPORT_BYTES
            .saturating_sub(priority.len())
            .min(REPORT_CRASH_CORE_BYTES);
        let (crash_core, crash_details) = crash_report.map_or_else(
            || (String::new(), String::new()),
            |report| crash_report_excerpt(report, crash_budget, &own),
        );
        let priority_budget = REPORT_BYTES - crash_core.len();
        let remaining = priority_budget.saturating_sub(priority.len());
        let skip = lines.len().saturating_sub(REPORT_LOG_LINES);
        let tail = lines[skip..].join("\n");
        // Preserve the latest bounded snapshot when earlier, handled errors fill their section.
        let freeze_bytes = freeze_frame
            .and_then(|_| tail.rfind("---- logs/arctic-freezes/"))
            .map_or(0, |start| (tail.len() - start).min(REPORT_LOG_BYTES));
        let tail_bytes = REPORT_LOG_BYTES
            .saturating_sub(early.len())
            .max(REPORT_LOG_BYTES / 4)
            .max(freeze_bytes);
        let mut text = format!("\n{}", tail_of(&tail, tail_bytes));
        text.push_str("\n\n---- initial output ----\n");
        text.push_str(head_of(&all, 2 * 1024));
        let launcher_compact = launcher_lines
            .iter()
            .rev()
            .take(REPORT_LAUNCHER_LINES)
            .rev()
            .map(|line| compact_launch(line))
            .collect::<Vec<_>>()
            .join("\n");
        let reserved = launcher_compact.len().min(REPORT_LAUNCHER_BYTES)
            + java.map_or(0, |s| s.len().min(REPORT_JAVA_BYTES));
        let evidence_budget = remaining.saturating_sub(text.len() + reserved + 512);
        let evidence = diagnostic_evidence(&all, evidence_budget);
        if !evidence.is_empty() {
            text.push_str("\n\n---- diagnostics ----\n");
            text.push_str(head_of(&evidence, evidence_budget));
        }
        if !launcher_lines.is_empty() {
            text.push_str("\n\n---- launcher log ----\n");
            text.push_str(tail_of(&launcher_compact, REPORT_LAUNCHER_BYTES));
        }
        if let Some(java) = java {
            text.push_str("\n\n---- java ----\n");
            text.push_str(head_of(java, REPORT_JAVA_BYTES));
        }
        let text = scrub(&text, &own);
        let text = head_of(&text, remaining);
        let crash_details = head_of(
            &crash_details,
            (remaining - text.len()).min(REPORT_CRASH_BYTES.saturating_sub(crash_core.len())),
        );
        let title = exit_title(title, &all, java.unwrap_or(""));
        let title =
            if title.is_empty() || title == "unknown" || title.starts_with("Game exited with code")
            {
                if let Some((d, _)) = wrong_java {
                    d.title.clone()
                } else {
                    quilt_load_error(&diagnosis_text).map_or(title, |(d, _)| d.title)
                }
            } else {
                title
            };
        let title = if title.is_empty()
            || title == "unknown"
            || title.starts_with("Game exited with code")
            || title.starts_with("A mod needs ")
        {
            fabric_wrong_game(&diagnosis_text).map_or(title, |(d, _)| d.title)
        } else {
            title
        };
        let title = diagnosis
            .as_ref()
            .filter(|(d, _)| {
                d.title == crate::crash_suspect::UNASSIGNED_ENTITY_TITLE
                    || d.title == crate::crash_suspect::THREADING_TITLE
                    || (d.title.ends_with(crate::crash_conflicts::CONFIG_CORRUPT)
                        && !title.ends_with(crate::crash_conflicts::CONFIG_CORRUPT))
            })
            .map_or(title, |(d, _)| d.title.clone());
        let title = if title == crate::crash_suspect::THREADING_TITLE {
            title
        } else {
            forge_dependency_diagnosis(&diagnosis_text)
                .or_else(|| registry_sync_diagnosis(&diagnosis_text))
                .map_or(title, |(d, _)| d.title)
        };
        let title = java_fatal_diagnosis(&diagnosis_text)
            .or_else(|| runtime_diagnosis(&diagnosis_text))
            .or_else(|| native_diagnosis(&diagnosis_text))
            .map_or(title, |(d, _)| d.title);
        let title = if crash_report.is_none()
            && (title.is_empty()
                || title == "unknown"
                || title.starts_with("Game exited with code")
                || title == "The game froze and was closed")
        {
            hang_diagnosis(&diagnosis_text, None).map_or(title, |(d, _)| d.title)
        } else {
            title
        };
        Self {
            launcher: env!("CARGO_PKG_VERSION").to_owned(),
            game: game.to_owned(),
            loader: loader.to_owned(),
            os: std::env::consts::OS.to_owned(),
            title: scrub(
                if diagnosis
                    .as_ref()
                    .is_some_and(|(d, _)| d.title == DISK_FULL_TITLE)
                {
                    DISK_FULL_TITLE
                } else if failed_save {
                    SAVE_FAILED_TITLE
                } else if sodium_driver.is_some() {
                    SODIUM_DRIVER_TITLE
                } else if orderly_shutdown(&all)
                    && crash_report.is_none()
                    && diagnose(&all).is_none()
                {
                    "Non-crash: Minecraft stopped normally"
                } else {
                    &title
                },
                &own,
            ),
            // Mod ids/versions are public loader metadata. Keep version strings verbatim;
            // the generic scrubber would mistake four-part versions for IP addresses.
            log: format!(
                "{}{text}{crash_core}{crash_details}",
                head_of(&priority, priority_budget)
            ),
            ram_total_mb,
            ram_free_mb,
            contact: String::new(),
        }
    }
}

pub(crate) fn minecraft_report_section(line: &str) -> Option<&str> {
    line.trim().strip_prefix("-- ")?.strip_suffix(" --")
}

/// Budget the exception and diagnostic sections before spending space on supplemental logs.
fn crash_report_excerpt(report: &str, budget: usize, own: &Own) -> (String, String) {
    const HEADER: &str = "\n\n---- crash report ----\n";
    let Some(mut budget) = budget.checked_sub(HEADER.len()) else {
        return (String::new(), String::new());
    };
    let mut excerpt = HEADER.to_owned();
    if !report.contains("---- Minecraft Crash Report ----") {
        excerpt.push_str(head_of(&scrub(report, own), budget.min(REPORT_CRASH_BYTES)));
        return (excerpt, String::new());
    }

    // The preamble holds the complete exception chain. Mods can put their diagnostic fields
    // inside otherwise optional sections, with the value on more deeply indented lines.
    let mut required = vec![String::new()];
    let mut details = String::new();
    let mut required_section = true;
    let mut section = "";
    let mut field_indent = None;
    for line in report.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some(name) = minecraft_report_section(line) {
            section = name;
            required_section = matches!(name, "Head" | "Thread dumps")
                || name.starts_with("Mixins in Stack")
                || matches!(name, "Suspected Mod" | "Suspected Mods");
            field_indent = None;
            if required_section {
                required.push(String::new());
            }
        } else if required_section {
            // Keep the start of each cause/thread if the stacks themselves need trimming.
            if trimmed.starts_with("Caused by: ")
                || (section == "Thread dumps" && trimmed.ends_with(':') && trimmed != "Details:")
            {
                required.push(String::new());
            }
        } else {
            let indent = line.len() - line.trim_start().len();
            if trimmed.split_once(':').is_some_and(|(name, _)| {
                name.starts_with("Mixins in Stack")
                    || matches!(name, "Suspected Mod" | "Suspected Mods")
            }) {
                field_indent = Some(indent);
                required.push(String::new());
            } else if !trimmed.is_empty() && field_indent.is_some_and(|field| indent <= field) {
                field_indent = None;
            }
        }
        if required_section || field_indent.is_some() {
            required.last_mut().unwrap().push_str(line);
        } else {
            details.push_str(line);
        }
    }

    let mut blocks: Vec<_> = required.iter().map(|part| scrub(part, own)).collect();
    // If the diagnostic sections exceed their budget, preserve the short sections in full
    // and split the remaining space between the oversized stacks.
    let mut by_size: Vec<_> = (0..blocks.len()).collect();
    by_size.sort_unstable_by_key(|&i| blocks[i].len());
    let count = by_size.len();
    for (rank, i) in by_size.into_iter().enumerate() {
        let part = &mut blocks[i];
        let allowance = budget / (count - rank);
        if part.len() > allowance {
            const CUT: &str = "\n[crash report section truncated]\n";
            if allowance > CUT.len() {
                let prefix = head_of(part, allowance - CUT.len());
                let end = prefix.rfind('\n').unwrap_or(prefix.len());
                part.truncate(end);
                part.push_str(CUT);
            } else {
                part.clear();
            }
        }
        budget -= part.len();
    }
    for part in blocks {
        excerpt.push_str(&part);
    }
    if !details.is_empty() {
        details.insert(0, '\n');
    }
    (excerpt, scrub(&details, own))
}

/// "---- memory ----\n16384 MB total, 4096 MB free\n", with "unknown" for what couldn't be read.
fn memory_section(total_mb: Option<u64>, free_mb: Option<u64>) -> String {
    let mb = |v: Option<u64>| v.map_or_else(|| "unknown".to_owned(), |v| format!("{v} MB"));
    format!(
        "\n---- memory ----\n{} total, {} free\n",
        mb(total_mb),
        mb(free_mb)
    )
}

/// Resolve inventory from the untrimmed Fabric report, or its startup list when Minecraft
/// died too early to write one. The agent sidecar also covers failures swallowed by preloading.
pub(crate) fn compact_mod_inventory(output: &str, report: &str) -> String {
    let mut mods = std::collections::BTreeMap::new();
    static STARTUP_MOD: OnceLock<Regex> = OnceLock::new();
    let startup = STARTUP_MOD
        .get_or_init(|| Regex::new(r"^\s*(?:- |[| \\+]*-- )([a-z][a-z0-9_-]*) (.+)$").unwrap());
    let mut in_startup = false;
    for line in output.lines() {
        if line.contains("Loading ") && line.ends_with(" mods:") {
            in_startup = true;
            continue;
        }
        if in_startup {
            if let Some(c) = startup.captures(line) {
                mods.insert(c[1].to_owned(), c[2].to_owned());
            } else if !line.trim().is_empty() {
                in_startup = false;
            }
        }
    }
    let mut in_report = false;
    for line in report.lines() {
        if line.trim() == "Fabric Mods:" {
            in_report = true;
            continue;
        }
        if !in_report || line.trim().is_empty() {
            continue;
        }
        let Some((id, detail)) = line.trim().split_once(':') else {
            break;
        };
        if id.is_empty()
            || !id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-')
        {
            break;
        }
        if let Some(version) = detail.split_whitespace().last() {
            mods.insert(id.to_owned(), version.to_owned());
        }
    }
    // Prefer Fabric's live resolved metadata if the failure sidecar supplies it.
    for line in output.lines() {
        if let Some(entry) = line
            .split_once("[Arctic mixin mod] ")
            .map(|(_, entry)| entry)
            && let Some((id, version)) = entry.split_once(' ')
        {
            mods.insert(id.to_owned(), version.to_owned());
        }
    }
    let mut out = String::new();
    for (id, version) in mods {
        out.push_str(&format!("{id} {version}\n"));
    }
    out
}

/// Keep very large inventories complete within the existing 48 KB upload limit. Typical
/// packs stay readable; exceptionally large packs carry a lossless gzip+base64 section.
fn mod_inventory_section(mods: &str) -> String {
    if mods.len() > 24 * 1024 {
        use base64::Engine as _;
        use std::io::Write as _;
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        if gzip.write_all(mods.as_bytes()).is_ok()
            && let Ok(bytes) = gzip.finish()
        {
            return format!(
                "---- full mod list (gzip+base64; decoded lines are id version) ----\n{}\n",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            );
        }
    }
    format!("---- full mod list (id version) ----\n{mods}")
}

/// Deduplicate stderr/log/sidecar copies, preserving input method signatures and all target mixins.
fn mixin_failure_evidence(output: &str) -> String {
    let mut blocks = Vec::new();
    let mut current = String::new();
    for line in output.lines() {
        let Some((_, context)) = line.split_once("[Arctic mixin] ") else {
            continue;
        };
        if context == "failure begin" && !current.is_empty() {
            blocks.push(std::mem::take(&mut current));
        }
        if context == "failure begin" || !current.is_empty() {
            current.push_str(context);
            current.push('\n');
            if context == "failure end" {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            blocks.push(format!("{context}\n"));
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    let mut seen = std::collections::HashSet::new();
    blocks
        .into_iter()
        .filter(|block| seen.insert(block.clone()))
        .collect()
}

fn registry_id_error(line: &str) -> bool {
    static MISMATCH: OnceLock<Regex> = OnceLock::new();
    MISMATCH
        .get_or_init(|| {
            Regex::new(r"(?i)did not get ID it asked for|override did not have an associated owner|\bids?\b.*(?:mismatch|do not match|did not match|out of sync)|mismatch.*\bids?\b|expected.*(?:got|actual)").unwrap()
        })
        .is_match(line)
}

/// Select recent registry records from the full latest.log before its ordinary tail is taken.
/// Deduplicated lines stay within a fixed byte budget; unusually long records are clipped.
pub fn registry_failure_evidence(output: &str) -> String {
    let mut selected = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut bytes = 0;
    for line in output.lines().rev() {
        let warning = line.contains("/WARN]") || line.contains("/ERROR]");
        if !warning
            || !(line.contains("net.minecraftforge.registries")
                || (line.contains("Registry") && registry_id_error(line)))
            || !seen.insert(line)
        {
            continue;
        }
        let line = head_of(line, 2048);
        if bytes + line.len() + 1 > REPORT_REGISTRY_BYTES {
            break;
        }
        bytes += line.len() + 1;
        selected.push(line);
    }
    selected.reverse();
    let mut out = selected.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// Compact only launch lines, before taking the byte tail. Java's separate summary stays intact.
fn compact_launch(line: &str) -> String {
    if !line.contains("launching") {
        return line.to_owned();
    }
    static CP: OnceLock<Regex> = OnceLock::new();
    CP.get_or_init(|| {
        Regex::new(r#"((?:-cp|-classpath|--class-path)\s+)(?:"[^"\n]*"|'[^'\n]*'|\S+)"#).unwrap()
    })
    .replace_all(line, "${1}<classpath>")
    .into_owned()
}

/// An unrecovered watchdog stall, or Windows hang termination with a fresh freeze snapshot.
pub fn freeze_evidence(text: &str) -> bool {
    if text.contains("0xCFFFFFFF") && text.contains("---- logs/arctic-freezes/") {
        return true;
    }
    let snapshot = text
        .rsplit("---- logs/arctic-freezes/")
        .next()
        .unwrap_or(text);
    if snapshot.contains("duration:") && snapshot.contains("(still frozen when written)") {
        return true;
    }
    if text.contains("Hang at exit:") && text.contains("no recovery recorded") {
        return true;
    }
    text.rfind("[Arctic diagnostics] hang begin")
        .is_some_and(|hang| {
            text.rfind("[Arctic diagnostics] hang recovered")
                .is_none_or(|recovered| recovered < hang)
        })
}

/// Require explicit shutdown evidence; a Stopping! line alone does not explain exit(1).
pub fn orderly_shutdown(text: &str) -> bool {
    text.contains("Stopping!")
        && (text.contains("All dimensions are saved")
            || text.contains("All chunks are saved")
            || text.contains("Saved the game"))
        && text.contains("[Arctic diagnostics] minecraft stop complete")
        && text.contains("[Arctic diagnostics] exit caller vanilla")
        && !text.contains("[Arctic diagnostics] exit caller other")
        && !freeze_evidence(text)
}

/// Most of the first-exceptions section (bytes), and how many distinct entries it keeps.
const FIRST_EXCEPTIONS_BYTES: usize = 8 * 1024;
const FIRST_EXCEPTIONS: usize = 3;
/// Stack frames kept per exception in a cause chain, and chain links kept per entry.
const FIRST_EXCEPTION_FRAMES: usize = 25;
const FIRST_EXCEPTION_CAUSES: usize = 6;

fn exception_record_boundary(line: &str) -> bool {
    line.starts_with('[') || line.starts_with("Exception in thread \"") || line.starts_with("---- ")
}

fn services_key_failure(line: &str) -> bool {
    line.contains("https://api.minecraftservices.com/publickeys")
        && (line.contains("com.mojang.authlib.exceptions.") || line.contains("Failed to request"))
}

fn background_auth_record<'a>(header: &'a str, body: impl Iterator<Item = &'a str>) -> bool {
    if services_key_failure(header)
        || header.rsplit_once(']').is_some_and(|(_, message)| {
            matches!(
                message.trim_start_matches([' ', ':']),
                "Failed to fetch user properties" | "Failed to fetch Realms feature flags"
            )
        })
    {
        return true;
    }
    static THREAD: OnceLock<Regex> = OnceLock::new();
    static AUTH: OnceLock<Regex> = OnceLock::new();
    let thread = THREAD.get_or_init(|| {
        Regex::new(r#"(?i)^(?:(?:\[[^\]]*\]\s*)?\[(?:Download|Realms)[^\]]*/(?:ERROR|FATAL)\]|Exception in thread "(?:Download|Realms)[^"]*")"#).unwrap()
    });
    if !thread.is_match(header) {
        return body.into_iter().any(services_key_failure);
    }
    let auth = AUTH.get_or_init(|| Regex::new(r"(?i)\b(?:401|authlib|InvalidCredentialsException|RealmsServiceException|Failed to fetch Realms feature flags|Couldn't connect to realms)\b").unwrap());
    std::iter::once(header)
        .chain(body)
        .any(|line| !line.trim_start().starts_with("at ") && auth.is_match(line))
}

/// The first distinct ERROR/FATAL log entries that carry a stack trace, from the whole log, so a
/// root cause that scrolled out of the tail (e.g. a join-time NPE that later leaves `player`
/// null) is still reported. Repeats of the same trace are kept once.
fn first_exceptions(text: &str) -> String {
    let mut seen: Vec<String> = Vec::new();
    let mut out = String::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if !(line.starts_with('[') && (line.contains("/ERROR]") || line.contains("/FATAL]"))) {
            continue;
        }
        let mut body = Vec::new();
        while let Some(next) = lines.next_if(|l| !exception_record_boundary(l)) {
            body.push(next);
        }
        if background_auth_record(line, body.iter().copied())
            || !body.iter().any(|l| l.trim_start().starts_with("at "))
        {
            continue;
        }
        let message = line.split_once("]: ").map_or(line, |(_, m)| m);
        let frames: Vec<&str> = body
            .iter()
            .copied()
            .filter(|l| l.trim_start().starts_with("at "))
            .take(4)
            .collect();
        let key = format!(
            "{message}\n{}\n{}",
            body.first().copied().unwrap_or(""),
            frames.join("\n")
        );
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let mut entry = String::from(line);
        entry.push('\n');
        let (mut frames, mut causes) = (0, 0);
        for l in body {
            let t = l.trim_start();
            if t.starts_with("at ") {
                frames += 1;
                if frames > FIRST_EXCEPTION_FRAMES || causes > FIRST_EXCEPTION_CAUSES {
                    continue;
                }
            } else if t.starts_with("Caused by:") {
                frames = 0;
                causes += 1;
                if causes > FIRST_EXCEPTION_CAUSES {
                    continue;
                }
            } else if t.starts_with("...") || causes > FIRST_EXCEPTION_CAUSES {
                continue;
            }
            entry.push_str(head_of(l, 512));
            entry.push('\n');
        }
        if out.len() + entry.len() > FIRST_EXCEPTIONS_BYTES {
            // A single oversized first entry is cut rather than dropped.
            if out.is_empty() {
                out.push_str(head_of(&entry, FIRST_EXCEPTIONS_BYTES));
            }
            break;
        }
        out.push_str(&entry);
        if seen.len() >= FIRST_EXCEPTIONS {
            break;
        }
    }
    out
}

/// Preserve the latest dump and exit trace even when they are outside the log tail.
fn diagnostic_evidence(text: &str, budget: usize) -> String {
    let mut out = String::new();
    if let Some(start) = text.rfind("[Arctic diagnostics] hang begin") {
        let dump = &text[start..];
        let end = dump
            .find("[Arctic diagnostics] hang end")
            .map_or(dump.len(), |n| n + "[Arctic diagnostics] hang end".len());
        let limit = budget.saturating_sub(4 * 1024);
        out.push_str(head_of(&dump[..end], limit));
        if end > limit {
            out.push_str(
                "\n[dump truncated to fit 48 KB; full dump saved locally as arctic-hang.txt]\n",
            );
        }
        out.push('\n');
        if let Some(recovered) = dump
            .lines()
            .find(|line| line.contains("[Arctic diagnostics] hang recovered"))
        {
            out.push_str(recovered);
            out.push('\n');
        }
    }
    for line in text.lines().filter(|l| {
        l.contains("[Arctic diagnostics]")
            && !l.starts_with(VANILLA_CRASH)
            && !l.contains("hang ")
            && !l.contains("thread ")
    }) {
        if out.len() < budget.saturating_sub(1024) {
            out.push_str(line);
            out.push('\n');
        }
    }
    let packs: Vec<_> = text
        .lines()
        .filter(|l| {
            l.contains("Couldn't load") && l.contains("pack metadata")
                || l.contains("Missing metadata in pack")
                || l.contains("MalformedJsonException") && l.contains("$.pack")
        })
        .collect();
    if !packs.is_empty() {
        out.push_str("Broken resource-pack metadata: ");
        out.push_str(&packs.join(" | "));
    }
    out
}

fn exit_title(original: &str, text: &str, java: &str) -> String {
    if !original.is_empty()
        && original != "unknown"
        && !original.starts_with("The game stopped before")
    {
        return original.to_owned();
    }
    let caller = text.lines().find_map(|line| {
        line.split_once("[Arctic diagnostics] exit caller other ")
            .map(|(_, frame)| frame.split('<').next().unwrap_or(frame).trim())
    });
    static CODE: OnceLock<Regex> = OnceLock::new();
    let combined = format!("{java}\n{text}");
    let code = CODE
        .get_or_init(|| Regex::new(r"(?:exit code |status: )(-?\d+)").unwrap())
        .captures(&combined)
        .and_then(|c| c[1].parse::<i32>().ok());
    match (code, caller) {
        (Some(code), Some(frame)) => {
            format!("Game exited with code {code}: {}", head_of(frame, 180))
        }
        (Some(code), None) => format!("Game exited with code {code} (0x{:08X})", code as u32),
        _ => original.to_owned(),
    }
}

/// Codes are unsigned on Windows; 0xCFFFFFFF is not a documented NTSTATUS.
pub fn exit_description(code: i32) -> String {
    let name = match code as u32 {
        0xC0000005 => "STATUS_ACCESS_VIOLATION",
        0xC0000142 => "STATUS_DLL_INIT_FAILED",
        0xC0000135 => "STATUS_DLL_NOT_FOUND",
        0xC0000409 => "STATUS_STACK_BUFFER_OVERRUN / fail-fast",
        0xCFFFFFFF => {
            "undocumented Windows exit code; hang termination or native failure, not enough evidence to distinguish"
        }
        _ => "no known NTSTATUS name",
    };
    format!("{code} (0x{:08X}; {name})", code as u32)
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

/// A scrubbed log for staff, with repeated messages collapsed and both ends kept.
pub fn scrubbed_shared_log(text: &str, max: usize) -> String {
    shared_log(text, max, &Own::this_pc())
}

fn shared_log(text: &str, max: usize, own: &Own) -> String {
    // Redact before adding markers or cutting, while multiline secrets are still intact.
    let clean = scrub(text, own);
    let text = collapse_log_repeats(text, clean);
    if text.len() <= max {
        return text;
    }

    let cut_marker =
        |bytes: usize| format!("--- {} KB cut from the middle ---\n", bytes.div_ceil(1024));
    let marker_budget = cut_marker(text.len()).len();
    if max < marker_budget {
        return tail_of(&text, max).to_owned();
    }

    let head_limit = (60 * 1024).min(max / 5).min(max - marker_budget);
    let head_end = head_of(&text, head_limit).rfind('\n').map_or(0, |i| i + 1);
    let mut tail = tail_of(&text, max - marker_budget - head_end);
    let tail_start = text.len() - tail.len();
    // Keep complete tail lines when they fit, or the end of an oversized last line.
    if text.as_bytes()[tail_start - 1] != b'\n'
        && let Some((_, rest)) = tail.split_once('\n')
        && !rest.is_empty()
    {
        tail = rest;
    }
    let marker = cut_marker(text.len() - head_end - tail.len());
    format!("{}{marker}{tail}", &text[..head_end])
}

fn collapse_log_repeats(text: &str, clean: String) -> String {
    let lines: Vec<_> = clean.split_inclusive('\n').collect();
    // Different messages can become identical after redaction.
    let keys: Vec<_> = text
        .lines()
        .map(|line| {
            let (time, message) = log_timestamp(line);
            (time.is_some(), message)
        })
        .collect();
    if lines.len() != keys.len() {
        return clean;
    }
    let mut out = String::new();
    let mut start = 0;
    while start < lines.len() {
        // Try the shortest period first. A repeat must match the whole block.
        let width = (1..=4)
            .find(|&width| {
                start + 2 * width <= keys.len()
                    && keys[start..start + width] == keys[start + width..start + 2 * width]
            })
            .unwrap_or(1);
        let first_end = start + width;
        for line in &lines[start..first_end] {
            out.push_str(line);
        }
        let mut end = first_end;
        while end + width <= keys.len() && keys[start..first_end] == keys[end..end + width] {
            end += width;
        }
        if end > first_end {
            let last = lines[end - 1];
            let count = (end - first_end) / width;
            let (time, _) = log_timestamp(last);
            let ending = if last.ends_with("\r\n") {
                "\r\n"
            } else if last.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            if width == 1 {
                out.push_str(&format!("... repeated {count} more times"));
            } else {
                out.push_str(&format!(
                    "... preceding {width} lines repeated {count} more times"
                ));
            }
            if let Some(time) = time {
                out.push_str(&format!(" (until {time})"));
            }
            out.push_str(ending);
        }
        start = end;
    }
    out
}

fn log_timestamp(line: &str) -> (Option<&str>, &str) {
    if let [b'[', h1, h2, b':', m1, m2, b':', s1, s2, b']', ..] = line.as_bytes()
        && [h1, h2, m1, m2, s1, s2].into_iter().all(u8::is_ascii_digit)
    {
        (Some(&line[1..9]), &line[10..])
    } else {
        (None, line)
    }
}

/// The part of Java's `hs_err_pid` file worth sending: the header, the summary (without the command line,
/// which holds the game's arguments) and the thread that died. The process section onward (VM arguments,
/// environment variables, loaded libraries, system details) is left out: it describes the PC, not the crash.
pub fn trim_hs_err(text: &str) -> String {
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("--------------- ") && line.contains("P R O C E S S") {
            break;
        }
        let lower = line.trim_start().to_ascii_lowercase();
        if lower.starts_with("command line:")
            || lower.starts_with("java_command:")
            || lower.starts_with("jvm_args:")
        {
            continue;
        }
        out.push(line);
    }
    out.join(
        "
",
    )
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
            // The game's own start-up line (1.8.9 writes it to latest.log): the token and the player's id.
            (r"\(Session ID is [^)\r\n]*\)?", "(Session ID is <hidden>)"),
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
    const VANILLA_EXIT_398: &str = include_str!("../tests/fixtures/crash-398-vanilla-exit.log");
    const HANG_436: &str = include_str!("../tests/fixtures/crash-436-customrecipe-hang.log");
    const HANG_487: &str = include_str!("../tests/fixtures/crash-487-iris-shader-hang.log");
    const HANG_489: &str = include_str!("../tests/fixtures/crash-489-gl-fence-hang.log");
    const HANG_516: &str = include_str!("../tests/fixtures/crash-516-jei-hang.log");
    const FREEZE_440: &str = include_str!("../tests/fixtures/crash-440-create-world-freeze.log");
    const SODIUM_490: &str = include_str!("../tests/fixtures/crash-490-sodium-driver.log");

    #[test]
    fn sodium_driver_block_490_outranks_freeze_and_auth_noise() {
        let (noise, hang_and_exit) = SODIUM_490
            .split_once("[Arctic diagnostics] hang begin")
            .unwrap();
        let (hang, exit) = hang_and_exit
            .split_once("[Arctic diagnostics] exit Runtime.exit()")
            .unwrap();
        let driver_frame = "net.caffeinemc.mods.sodium.client.compatibility.checks.GraphicsDriverChecks.postContextInit";
        let hang_only = format!("{noise}[Arctic diagnostics] hang begin{hang}");
        assert!(first_java_error(SODIUM_490, None).is_none());
        let dir = tempfile::tempdir().unwrap();
        for log in [
            SODIUM_490.to_owned(),
            hang_only.clone(),
            format!("{noise}{exit}"),
        ] {
            let diagnosis = crate::crash_conflicts::diagnose_exit_in(&log, 1, dir.path()).unwrap();
            assert_eq!(diagnosis.title, SODIUM_DRIVER_TITLE);
            assert_eq!(
                diagnosis
                    .detail
                    .matches("Intel(R) HD Graphics 4000")
                    .count(),
                1
            );
            assert!(diagnosis.detail.contains("driver 10.18.10.4358"));
            assert!(diagnosis.detail.contains("Update your graphics driver"));
            assert!(diagnosis.detail.contains("dedicated GPU"));
            let report = Report::build(
                "1.21.11",
                "Fabric",
                "The game froze and was closed",
                &[&log],
                &[],
                None,
                None,
            );
            assert_eq!(report.title, SODIUM_DRIVER_TITLE);
            assert!(report.log.starts_with(&format!(
                "\n---- first error ----\nError: {SODIUM_DRIVER_TITLE}\nknot//{driver_frame}("
            )));
            assert!(report.log.contains(&format!(
                "---- diagnosis ----\n{SODIUM_DRIVER_TITLE}\nmatched: knot//{driver_frame}("
            )));
            assert!(report.log.contains(
                "Available display adapters include Intel(R) HD Graphics 4000 (driver 10.18.10.4358"
            ));
        }
        let recovered =
            format!("{hang_only}[Arctic diagnostics] hang recovered; render progress resumed");
        assert!(sodium_driver_diagnosis(&recovered).is_none());
        assert!(
            sodium_driver_diagnosis(&format!(
                "    at knot//{driver_frame}(GraphicsDriverChecks.java:25)"
            ))
            .is_none()
        );
    }

    #[test]
    fn sodium_driver_block_requires_a_graphics_driver_check() {
        let unrelated_dialog = SODIUM_490.replace(
            "compatibility.checks.GraphicsDriverChecks.postContextInit",
            "compatibility.checks.WindowsModuleChecks.checkModules",
        );
        assert!(sodium_driver_diagnosis(&unrelated_dialog).is_none());
    }

    #[test]
    fn sodium_driver_block_accepts_startup_checks_without_adapter_inventory() {
        let exit = "[Arctic diagnostics] exit caller other knot//net.caffeinemc.mods.sodium.client.compatibility.checks.GraphicsDriverChecks.preContextInit";
        let snapshot = "---- logs/arctic-freezes/freeze-now.txt ----\n\
                        duration: 3.00s (still frozen when written)\n\
                        == Where the time went (frames present in the most samples) ==\n\
                        100% net.caffeinemc.mods.sodium.client.compatibility.checks.GraphicsDriverChecks.postContextInit(GraphicsDriverChecks.java:25)\n";
        for log in [exit, snapshot] {
            let diagnosis = diagnose(log).unwrap();
            assert_eq!(diagnosis.title, SODIUM_DRIVER_TITLE);
            assert!(!diagnosis.detail.contains("Available display adapters"));
        }
    }

    #[test]
    fn create_world_freeze_groups_by_the_client_call_site() {
        let upload = |snapshot: &str| {
            Report::build(
                "1.21.11",
                "Fabric",
                "",
                &[
                    "[15:57:49] [Download-2/ERROR] Failed to fetch user properties",
                    "    com.mojang.authlib.exceptions.InvalidCredentialsException: Status: 401",
                    "    java.io.IOException: unrelated background error",
                    "---- logs/arctic-freezes/freeze-test.txt ----",
                    snapshot,
                ],
                &[],
                None,
                Some("exit code -805306369 (0xCFFFFFFF)"),
            )
        };
        fn fingerprint_input(report: &Report) -> &str {
            // The server reads Error: lines before falling back to the title.
            let line = report.log.lines().find(|line| !line.is_empty()).unwrap();
            assert!(line.starts_with("Error: Watchdog freeze at exit ["));
            assert!(!line.contains(|c: char| c.is_ascii_digit()));
            line
        }
        let report = upload(FREEZE_440);
        assert_eq!(
            report.title,
            "The game was closed while the Create World screen was loading"
        );
        assert_eq!(
            freeze_group_frame(FREEZE_440, FREEZE_440),
            Some("net.minecraft.class_525.method_64244")
        );
        let repeat = FREEZE_440
            .replace("3c4e4871", "abcdef12")
            .replace("class_525.java:374", "class_525.java:375")
            .replace("    at knot//", "    at app//")
            .replace(
                "100%  knot//net.minecraft.class_525",
                "100%  app//net.minecraft.class_525",
            );
        assert_eq!(
            fingerprint_input(&report),
            fingerprint_input(&upload(&repeat))
        );

        let other_call = FREEZE_440.replace("method_64244", "method_64245");
        let driver_hang = "Arctic: game froze for 3.0s, mostly in org.lwjgl.opengl.GL20C.nglGetProgramiv(Native Method)\nduration: 3.00s (still frozen when written)\n";
        for different in [other_call.as_str(), driver_hang] {
            assert_ne!(
                fingerprint_input(&report),
                fingerprint_input(&upload(different))
            );
        }
    }

    #[test]
    fn brief_menu_wait_does_not_attribute_the_freeze_to_arctic() {
        let snapshot = FREEZE_440.replace(
            "knot//net.minecraft.class_437.method_25423(class_437.java:326)",
            "knot//com.arcticlauncher.client.Example.open(Example.java:12)",
        );
        let diagnosis = diagnose(&snapshot).unwrap();
        assert!(diagnosis.title.contains("Create World"));
        assert!(diagnosis.mods.is_empty());
        assert!(!diagnosis.detail.contains("Arctic bug"));

        let fatal = "Exception in thread \"Render thread\" java.lang.IllegalStateException: crashed\n    at example.Game.stop(Game.java:42)";
        let report = Report::build(
            "1.21.11",
            "Fabric",
            "Crash",
            &[FREEZE_440, fatal],
            &[],
            None,
            None,
        );
        assert_eq!(report.title, "Crash");
        assert!(
            report
                .log
                .starts_with("\n---- first error ----\njava.lang.IllegalStateException: crashed")
        );
    }

    #[test]
    fn report_436_names_the_frozen_mod_over_the_optional_mixin_warning() {
        let (diagnosis, matched) = diagnose_match(HANG_436, None).unwrap();
        assert_eq!(diagnosis.title, "customrecipe froze the game");
        assert_eq!(diagnosis.mods, ["customrecipe"]);
        assert!(diagnosis.detail.contains("third-party code"));
        assert!(
            diagnosis
                .detail
                .contains("VanillaRecipesScreen.findLocalRecipes")
        );
        assert!(matched.contains("94%"));

        // The uploaded report can omit the snapshot header while retaining the samples.
        let start = HANG_436.find("---- logs/arctic-freezes/").unwrap();
        let end = HANG_436.find("samples: 97").unwrap();
        let trimmed = [&HANG_436[..start], &HANG_436[end..]].concat();
        let report = Report::build("26.3", "Fabric", "", &[&trimmed], &[], None, None);
        assert_eq!(report.title, diagnosis.title);
        assert!(
            report
                .log
                .contains("---- diagnosis ----\ncustomrecipe froze the game\n")
        );
        assert!(!report.title.contains("c2me"));

        let without_exit_summary = trimmed
            .lines()
            .filter(|l| !l.starts_with("Hang at exit:"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            diagnose(&without_exit_summary).unwrap().title,
            diagnosis.title
        );
    }

    #[test]
    fn report_487_explains_the_iris_freeze_and_skips_the_player_attributes_401() {
        let (log, launcher) = HANG_487.split_once("---- launcher log ----\n").unwrap();
        let (launcher, exit) = launcher.split_once("(exit code ").unwrap();
        let (diagnosis, matched) = diagnose_match(HANG_487, None).unwrap();
        assert_eq!(diagnosis.title, IRIS_SHADER_LOADING_TITLE);
        assert!(diagnosis.detail.contains("slower graphics cards"));
        assert!(matched.contains("iris.shaderpack.ShaderPack.<init>"));

        let (_, without_auth_header) = log.split_once('\n').unwrap();
        let console_trace = without_auth_header
            .lines()
            .map(|line| {
                if line.contains("path=/player/attributes,") {
                    "Caused by: com.mojang.authlib.exceptions.MinecraftClientHttpException: Status: 401"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        for log in [log, without_auth_header, &console_trace] {
            assert_eq!(first_java_error(log, None), None);
            let report = Report::build(
                "1.21.11",
                "Fabric",
                "The game froze and was closed",
                &log.lines().collect::<Vec<_>>(),
                &launcher.lines().collect::<Vec<_>>(),
                None,
                Some(&format!("(exit code {exit}")),
            );
            assert_eq!(report.title, IRIS_SHADER_LOADING_TITLE);
            assert!(report.log.contains("Error: Watchdog freeze at exit"));
            assert!(!report.log.contains("---- first error ----"));
            assert!(!report.log.contains("---- first exceptions ----"));
        }
    }

    #[test]
    fn report_516_explains_the_jei_recipe_loading_freeze() {
        let (diagnosis, matched) = diagnose_match(HANG_516, None).unwrap();
        assert_eq!(diagnosis.title, JEI_RECIPE_LOADING_TITLE);
        assert!(diagnosis.detail.contains("when you join a world"));
        assert!(diagnosis.detail.contains("several minutes"));
        assert_eq!(diagnosis.mods, ["jei"]);
        assert!(diagnosis.fix.is_none());
        assert!(matched.contains("AnvilRecipeMaker.getAnvilOutput"));

        let mods = tempfile::tempdir().unwrap();
        assert_eq!(
            crate::crash_conflicts::diagnose_exit_in(HANG_516, -805306369, mods.path()),
            Some(diagnosis)
        );
        let lines: Vec<_> = HANG_516.lines().collect();
        let report = Report::build(
            "1.20.1",
            "Fabric",
            "The game froze and was closed",
            &lines,
            &[],
            None,
            None,
        );
        assert_eq!(report.title, JEI_RECIPE_LOADING_TITLE);
        assert!(report.log.contains("Error: Watchdog freeze at exit"));
        assert!(report.log.contains(&format!(
            "---- diagnosis ----\n{JEI_RECIPE_LOADING_TITLE}\nmatched:"
        )));
        assert!(!report.log.contains("porting_lib_base failed"));
        assert!(!report.log.contains("failed to change the game"));
    }

    #[test]
    fn jei_recipe_loading_needs_an_active_client_stall() {
        let (log, dump) = HANG_516.split_once("---- diagnostics ----").unwrap();
        assert!(diagnose(log).is_none());
        assert!(diagnose(&format!("{dump}\n[Arctic diagnostics] hang recovered")).is_none());

        let worker_stack = dump.replace("\"Render thread\"", "\"Worker-Main-1\"");
        let client_stack = dump.replace("\"Render thread\"", "\"Client thread\"");
        let unrelated_frame = dump.replace("mezz.jei.", "example.not_jei.");
        for text in [worker_stack, unrelated_frame] {
            assert_eq!(
                diagnose(&text).unwrap().title,
                "The game froze and was closed"
            );
        }
        assert_eq!(
            diagnose(&client_stack).unwrap().title,
            JEI_RECIPE_LOADING_TITLE
        );

        let sample = "---- logs/arctic-freezes/freeze.txt ----\n\
            duration: 16.0s (still frozen when written)\n\
            == Where the time went (frames present in the most samples) ==\n\
            95%  mezz.jei.startup.JeiStarter.start(JeiStarter.java:42)";
        assert_eq!(diagnose(sample).unwrap().title, JEI_RECIPE_LOADING_TITLE);
        assert!(diagnose(&sample.replace(" (still frozen when written)", "")).is_none());
    }

    #[test]
    fn report_489_explains_the_gl_fence_hang_and_skips_background_requests() {
        let (log, exit) = HANG_489.rsplit_once("\n(exit code ").unwrap();
        let exit = format!("(exit code {exit}");
        let (diagnosis, matched) = diagnose_match(HANG_489, None).unwrap();
        assert_eq!(diagnosis.title, GRAPHICS_DRIVER_HANG_TITLE);
        assert!(
            diagnosis
                .detail
                .contains("hung while Minecraft was drawing")
        );
        assert!(diagnosis.detail.contains("Update your graphics driver"));
        assert!(
            diagnosis
                .detail
                .contains("Intel(R) HD Graphics 4000 (driver from 2015-12-21)")
        );
        assert_eq!(
            diagnosis
                .detail
                .matches("Intel(R) HD Graphics 4000")
                .count(),
            1
        );
        assert!(diagnosis.detail.contains("older Minecraft version"));
        assert!(diagnosis.detail.contains("close other GPU-heavy apps"));
        assert!(matched.contains("nglClientWaitSync(Native Method)"));
        let mods_dir = tempfile::tempdir().unwrap();
        let at_exit = crate::crash_conflicts::diagnose_exit_in(log, -805306369, mods_dir.path());
        assert_eq!(at_exit.unwrap(), diagnosis);

        let report = Report::build(
            "26.1",
            "Vanilla",
            "The game froze and was closed",
            &log.lines().collect::<Vec<_>>(),
            &[],
            None,
            Some(&exit),
        );
        assert_eq!(report.title, GRAPHICS_DRIVER_HANG_TITLE);
        assert!(report.log.contains(&diagnosis.detail));
        assert!(!report.log.contains("---- first error ----"));
        assert!(!report.log.contains("---- first exceptions ----"));

        let (_, clipped_realms) = log
            .split_once("Failed to fetch Realms feature flags\n")
            .unwrap();
        assert_eq!(first_java_error(clipped_realms, None), None);

        let (sampled, at_exit) = HANG_489.split_once("\nHang at exit:").unwrap();
        let (diagnosis, matched) = diagnose_match(sampled, None).unwrap();
        assert_eq!(diagnosis.title, GRAPHICS_DRIVER_HANG_TITLE);
        assert!(matched.contains("100%  knot//org.lwjgl.opengl.GL32C.nglClientWaitSync"));
        assert!(!diagnosis.detail.contains("older Minecraft version"));
        let minority = sampled.replace("100%", "50%");
        assert_eq!(
            diagnose(&minority).unwrap().title,
            "The game froze and was closed"
        );
        let recovered = sampled.replace(" (still frozen when written)", "");
        let recovered = format!("{recovered}\n{exit}");
        assert_eq!(
            diagnose(&recovered).unwrap().title,
            "The game froze and was closed"
        );

        let at_exit = format!("Hang at exit:{at_exit}").replace(
            "knot//org.lwjgl.opengl.GL32C.nglClientWaitSync(Native Method)",
            "org.lwjgl.glfw.GLFW.nglfwSwapBuffers(Native Method)",
        );
        assert_eq!(
            diagnose(&at_exit).unwrap().title,
            GRAPHICS_DRIVER_HANG_TITLE
        );
        let new_driver = at_exit.replace(
            "12-21-2015",
            &chrono::Utc::now().format("%m-%d-%Y").to_string(),
        );
        let diagnosis = diagnose(&new_driver).unwrap();
        assert!(diagnosis.detail.contains("driver from"));
        assert!(!diagnosis.detail.contains("older Minecraft version"));
    }

    #[test]
    fn iris_shader_loading_requires_current_client_frames() {
        let start = HANG_487.find("== Where the time went").unwrap();
        let stacks = HANG_487.find("== Distinct client-thread stacks").unwrap();
        let old_iris = [&HANG_487[..start], &HANG_487[stacks..]]
            .concat()
            .replace("net.irisshaders.iris.", "net.coderbot.iris.");
        assert_eq!(
            diagnose(&old_iris).unwrap().title,
            IRIS_SHADER_LOADING_TITLE
        );

        let recovered = HANG_487.replace(" (still frozen when written)", "");
        assert_ne!(
            diagnose(&recovered).unwrap().title,
            IRIS_SHADER_LOADING_TITLE
        );
        let later = format!(
            "{HANG_487}\n---- logs/arctic-freezes/freeze-later.txt ----\n\
             duration: 7.0s (still frozen when written)\n\
             == Likely awaited threads (from the first sample) ==\n\
             at knot//net.irisshaders.iris.Iris.reload(Iris.java:576)"
        );
        assert_eq!(
            diagnose(&later).unwrap().title,
            "The game froze and was closed"
        );
    }

    #[test]
    fn hang_attribution_requires_current_samples_and_one_dominant_owner() {
        let generic = "The game froze and was closed";
        assert_eq!(
            diagnose(&HANG_436.replace("94%", "59%")).unwrap().title,
            generic
        );
        let competing = HANG_436.replace(
            "   94%  knot//net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents",
            "   94%  knot//example.other.ClientTickEvents",
        );
        assert_eq!(diagnose(&competing).unwrap().title, generic);

        let start = HANG_436.find("Hang at exit:").unwrap();
        let recovered = HANG_436[..start].replace(" (still frozen when written)", "");
        assert!(diagnose(&recovered).is_none());
        let windows_exit = format!("{recovered}\n(exit code -805306369 (0xCFFFFFFF))");
        assert_eq!(diagnose(&windows_exit).unwrap().title, generic);
    }

    #[test]
    fn direct_arctic_and_polarium_hang_frames_are_our_bug() {
        for (package, name) in [
            ("com.arcticlauncher.client", "Arctic Client"),
            ("com.arcticlauncher.polarium", "Polarium"),
            ("example.unknown.client", "example.unknown.client"),
        ] {
            let text = HANG_436.replace("fr.zazac1.customrecipe.client", package);
            let diagnosis = diagnose(&text).unwrap();
            assert_eq!(diagnosis.title, format!("{name} froze the game"));
            assert_eq!(
                diagnosis.detail.contains("This is an Arctic bug."),
                name != package
            );
        }
    }

    #[test]
    fn real_crashes_keep_precedence_over_sampled_hangs() {
        for (failure, expected) in [
            (
                "(exit code -1073741819 (0xC0000005))",
                "Minecraft crashed in native code",
            ),
            (
                "java.lang.OutOfMemoryError: Java heap space",
                "Minecraft ran out of memory",
            ),
            (
                "InvalidMixinException: @Mixin target missing.Class was not found broken.mixins.json:BrokenMixin",
                "broken failed to change the game",
            ),
        ] {
            assert_eq!(
                diagnose(&format!("{HANG_436}\n{failure}")).unwrap().title,
                expected
            );
        }
    }

    #[test]
    fn crash_398_requires_vanilla_crash_and_failed_save_exit_code() {
        let mods = tempfile::tempdir().unwrap();
        let d =
            crate::crash_conflicts::diagnose_exit_in(VANILLA_EXIT_398, -2, mods.path()).unwrap();
        assert_eq!(d.title, SAVE_FAILED_TITLE);
        assert!(d.mods.is_empty());
        assert!(first_java_error(VANILLA_EXIT_398, None).is_none());
        assert_eq!(diagnose(VANILLA_EXIT_398).unwrap().title, SAVE_FAILED_TITLE);
        assert_eq!(
            crate::crash_conflicts::diagnose_in(VANILLA_EXIT_398, mods.path())
                .unwrap()
                .title,
            SAVE_FAILED_TITLE
        );
        let unix = VANILLA_EXIT_398.replace(
            "(exit code -2 (0xFFFFFFFE; no known NTSTATUS name))",
            "[Arctic diagnostics] process ended after 11000 ms; exit code 254 (0x000000FE)",
        );
        assert_eq!(diagnose(&unix).unwrap().title, SAVE_FAILED_TITLE);
        assert!(vanilla_crash_save_failed(&unix, 254).is_some());
        assert!(vanilla_crash_save_failed(VANILLA_EXIT_398, -1).is_none());
        assert!(
            vanilla_crash_save_failed(&VANILLA_EXIT_398.replace(".crash(", ".destroy("), -2)
                .is_none()
        );
        assert!(
            vanilla_crash_save_failed(
                &VANILLA_EXIT_398.replace("client.Minecraft.crash(", "class_310.method_1565("),
                -2,
            )
            .is_some()
        );
    }

    #[test]
    fn crash_398_prioritizes_captured_exception_and_keeps_save_evidence() {
        // The original upload had no Throwable. Supplement it with a simulated startup failure.
        let error = "java.lang.RuntimeException: Could not execute entrypoint stage 'client' due to errors, provided by 'example' at 'example.Client'!";
        let log = format!(
            "{VANILLA_EXIT_398}\n[Arctic diagnostics] vanilla crash: Initializing game\n\
             {VANILLA_CRASH_TRACE}{error}\n\
             {VANILLA_CRASH_TRACE}\tat knot//net.fabricmc.loader.impl.FabricLoaderImpl.invokeEntrypoints(FabricLoaderImpl.java:1)\n\
             {VANILLA_CRASH_TRACE}Caused by: java.lang.ClassNotFoundException: org.lwjgl.Optional\n\
             {VANILLA_CRASH_TRACE}\tat knot//example.Client.onInitializeClient(Client.java:10)\n\
             [Arctic diagnostics] vanilla crash save failed: <home>/game/crash-reports/crash.txt\n\
             [Arctic diagnostics] vanilla crash save trace java.nio.file.AccessDeniedException: <home>/game/crash-reports/crash.txt\n{}",
            "[main/INFO]: shutdown output\n".repeat(200)
        );
        let mods = tempfile::tempdir().unwrap();
        let d = crate::crash_conflicts::diagnose_exit_in(&log, -2, mods.path()).unwrap();
        assert_eq!(d.title, SAVE_FAILED_TITLE);
        assert!(d.detail.contains("A game library couldn't be loaded"));
        assert!(d.repair);
        let mixin = log.replace(
            "java.lang.ClassNotFoundException: org.lwjgl.Optional",
            "org.spongepowered.asm.mixin.injection.throwables.InjectionError: example.mixins.json from mod example",
        );
        let d = crate::crash_conflicts::diagnose_exit_in(&mixin, -2, mods.path()).unwrap();
        assert_eq!(d.title, SAVE_FAILED_TITLE);
        assert_eq!(d.mods, ["example"]);
        assert!(d.detail.contains("example failed to change the game"));
        let report = Report::build("26.1.2", "fabric", "unknown", &[&log], &[], None, None);
        assert_eq!(report.title, SAVE_FAILED_TITLE);
        assert!(report.log.contains("A game library couldn't be loaded"));
        assert!(
            report
                .log
                .starts_with(&format!("\n---- first error ----\n{error}\n"))
        );
        assert!(
            report.log.contains(
                "matched: Caused by: java.lang.ClassNotFoundException: org.lwjgl.Optional"
            )
        );
        assert!(report.log.contains("vanilla crash: Initializing game"));
        assert!(
            report
                .log
                .contains("vanilla crash trace \tat knot//example")
        );
        assert!(
            report
                .log
                .contains("save trace java.nio.file.AccessDeniedException: <home>")
        );
        assert!(report.log.len() < 48 * 1024);
    }

    const SHADERC_MISSING_515: &str = "\
java.lang.UnsatisfiedLinkError: Failed to locate library: shaderc.dll\n\
    \tat org.lwjgl.util.shaderc.Shaderc.<clinit>(Shaderc.java:22)\n\
    \tat com.mojang.blaze3d.platform.NativeLibrariesBootstrap.loadShaderc(NativeLibrariesBootstrap.java:325)";

    #[test]
    fn crash_515_disk_full_outranks_missing_shaderc_and_failed_save() {
        // The uploaded log redacts the home directory. Use a sample drive for the local log.
        let log = format!(
            "{SHADERC_MISSING_515}\n{}",
            r"[07:03:12] [main/ERROR] Could not save crash report to D:\Game\crash-reports\crash-2026-11-24_07.03.12-client.txt
    java.io.IOException: There is not enough space on the disk
[Arctic diagnostics] exit caller vanilla knot//net.minecraft.client.Minecraft.crash(Minecraft.java:1051)
[Arctic diagnostics] process ended after 8658 ms; exit code -2 (0xFFFFFFFE; no known NTSTATUS name)"
        );
        let mods = tempfile::tempdir().unwrap();
        let diagnosis = diagnose(&log).unwrap();
        assert_eq!(diagnosis.title, DISK_FULL_TITLE);
        assert!(diagnosis.detail.contains("the D: drive"));
        assert!(
            diagnosis
                .detail
                .contains("Free up some space and try again.")
        );
        assert_eq!(diagnosis.fix, None);
        assert!(!diagnosis.repair);
        assert_eq!(
            crate::crash_conflicts::diagnose_exit_in(&log, -2, mods.path()),
            Some(diagnosis.clone())
        );
        assert_eq!(
            crate::crash_conflicts::diagnose_in(&log, mods.path()),
            Some(diagnosis)
        );
        let report = Report::build(
            "26.2",
            "vanilla",
            SAVE_FAILED_TITLE,
            &[&log],
            &[],
            None,
            None,
        );
        assert_eq!(report.title, DISK_FULL_TITLE);
        assert!(
            report
                .log
                .contains("matched: java.io.IOException: There is not enough space on the disk")
        );
    }

    #[test]
    fn disk_full_matches_unix_and_java_errors_over_version_errors() {
        let version_error = "---- Minecraft Crash Report ----\n\njava.lang.NoSuchMethodError: 'void example.Mod.start()'";
        assert_eq!(
            diagnose(version_error).unwrap().title,
            "A mod is for a different version"
        );
        for error in [
            // OpenJDK on Linux writing to /dev/full.
            "Exception in thread \"main\" java.io.IOException: No space left on device\n\tat java.base/java.io.FileOutputStream.write(Native Method)",
            "[WARN] arctic session file: I/O error at /home/player/game/config/arctic-session.json.tmp: write failed (os error 28)",
            "java.io.IOException: disk full",
        ] {
            let log = format!("{version_error}\n{error}");
            let diagnosis = diagnose(&log).unwrap();
            assert_eq!(diagnosis.title, DISK_FULL_TITLE);
            assert!(diagnosis.detail.contains("the drive is out of space"));
        }
    }

    #[test]
    fn disk_full_matches_windows_os_error_and_its_drive() {
        let launcher_line = r"[WARN] arctic session file: I/O error at e:\Game\config\arctic-session.json.tmp: write failed (os error 112)";
        let report = Report::build(
            "1.8.9",
            "vanilla",
            "unknown",
            &[SHADERC_MISSING_515],
            &[launcher_line],
            None,
            None,
        );
        assert_eq!(report.title, DISK_FULL_TITLE);
        assert!(
            diagnose(launcher_line)
                .unwrap()
                .detail
                .contains("the E: drive")
        );
    }

    #[test]
    fn missing_shaderc_without_disk_full_keeps_its_diagnosis() {
        assert!(diagnose(SHADERC_MISSING_515).is_none());
        for other_error in ["os error 1120", "os error 280"] {
            assert!(diagnose(&format!("{SHADERC_MISSING_515}\n{other_error}")).is_none());
        }
    }

    #[test]
    fn crash_510_recommends_updating_fabric_loader_before_the_linkage_diagnosis() {
        let log = format!(
            "[18:14:53] [main/INFO] Loading Minecraft 1.21 with Fabric Loader 0.18.4\n{MIXINEXTRAS_CRASH_510}"
        );
        let mods = tempfile::tempdir().unwrap();
        let diagnosis = crate::crash_conflicts::diagnose_exit_in(&log, -1, mods.path()).unwrap();
        assert_eq!(diagnosis.title, "MixinExtras crashed on Fabric");
        assert!(diagnosis.detail.contains("Fabric Loader 0.18.4"));
        assert!(diagnosis.detail.contains("Try updating Fabric Loader"));
        assert_eq!(diagnosis.fix, Some(Fix::UpdateLoader));
        let (_, matched) = diagnose_match(&log, None).unwrap();
        assert!(matched.starts_with("java.lang.ClassCastException: class java.util.ArrayList"));

        let failed_save = format!(
            "{log}\n[Arctic diagnostics] exit caller vanilla net.minecraft.client.Minecraft.crash(Minecraft.java:1)\n(exit code -2)"
        );
        let diagnosis =
            crate::crash_conflicts::diagnose_exit_in(&failed_save, -2, mods.path()).unwrap();
        assert_eq!(diagnosis.title, SAVE_FAILED_TITLE);
        assert!(diagnosis.detail.contains("Fabric Loader 0.18.4"));
        assert_eq!(diagnosis.fix, Some(Fix::UpdateLoader));
        assert_eq!(diagnose_match(&failed_save, None).unwrap().1, matched);
    }

    // Trimmed from the captured startup trace. No local paths are included.
    const MIXINEXTRAS_CRASH_510: &str = "[Arctic diagnostics] vanilla crash: Initializing game
[Arctic diagnostics] vanilla crash trace \tSuppressed: java.lang.NoClassDefFoundError: Could not initialize class net.fabricmc.fabric.api.networking.v1.ServerConfigurationConnectionEvents
[Arctic diagnostics] vanilla crash trace \tCaused by: java.lang.RuntimeException: Mixin transformation of net.minecraft.server.MinecraftServer failed
[Arctic diagnostics] vanilla crash trace \t\tat net.fabricmc.loader.impl.launch.knot.KnotClassDelegate.getPostMixinClassByteArray$arctic(KnotClassDelegate.java:440)
[Arctic diagnostics] vanilla crash trace \tCaused by: java.lang.ClassCastException: class java.util.ArrayList cannot be cast to class org.objectweb.asm.tree.AnnotationNode (java.util.ArrayList is in module java.base of loader 'bootstrap'; org.objectweb.asm.tree.AnnotationNode is in unnamed module of loader 'app')
[Arctic diagnostics] vanilla crash trace \t\tat knot//com.llamalad7.mixinextras.wrapper.factory.FactoryRedirectWrapperMixinTransformer.transform(FactoryRedirectWrapperMixinTransformer.java:18)
[Arctic diagnostics] vanilla crash trace \t\tat knot//com.llamalad7.mixinextras.transformer.MixinTransformerExtension.preApply(MixinTransformerExtension.java:40)";

    #[test]
    fn mixinextras_cast_accepts_a_plain_fabric_stack_on_another_game_version() {
        let trace = MIXINEXTRAS_CRASH_510
            .replace(
                "net.minecraft.server.MinecraftServer",
                "net.minecraft.client.gui.MapRenderer",
            )
            .lines()
            .filter_map(|line| line.strip_prefix(VANILLA_CRASH_TRACE))
            .filter(|line| !line.contains("FactoryRedirectWrapperMixinTransformer"))
            .collect::<Vec<_>>()
            .join("\n");
        let log = format!("[main/INFO]: Loading Minecraft 26.2 with Fabric Loader 0.19.4\n{trace}");
        let diagnosis = diagnose(&log).unwrap();
        assert!(diagnosis.detail.contains("Fabric Loader 0.19.4"));
        assert_eq!(diagnosis.fix, Some(Fix::UpdateLoader));
    }

    #[test]
    fn mixinextras_cast_requires_fabric_startup_evidence() {
        for startup in [
            "",
            "Loading Minecraft 1.21 with Quilt Loader 0.28.1",
            "ModLauncher running",
        ] {
            let log = format!("{startup}\n{MIXINEXTRAS_CRASH_510}");
            assert_ne!(diagnose(&log).unwrap().fix, Some(Fix::UpdateLoader));
        }
    }

    #[test]
    fn mixinextras_cast_requires_the_cast_and_a_frame_in_the_same_stack() {
        let wrong_cast =
            MIXINEXTRAS_CRASH_510.replace("asm.tree.AnnotationNode", "asm.tree.ClassNode");
        let unrelated_frames =
            MIXINEXTRAS_CRASH_510.replace("com.llamalad7.mixinextras", "example.mod");
        let split_stack = MIXINEXTRAS_CRASH_510.replace(
            "[Arctic diagnostics] vanilla crash trace \t\tat knot//com.llamalad7.mixinextras.wrapper",
            "[Arctic diagnostics] vanilla crash trace \tCaused by: java.lang.IllegalStateException\n[Arctic diagnostics] vanilla crash trace \t\tat knot//com.llamalad7.mixinextras.wrapper",
        );
        for trace in [wrong_cast, unrelated_frames, split_stack] {
            let log = format!("Loading Minecraft 1.21 with Fabric Loader 0.18.4\n{trace}");
            let diagnosis = diagnose(&log).unwrap();
            assert_eq!(diagnosis.title, "A mod is for a different version");
            assert_eq!(diagnosis.fix, None);
        }
    }

    #[test]
    fn crash_380_prefers_the_runtime_dll_over_the_debug_screen_error() {
        let text = include_str!("../tests/fixtures/crash-380-runtime-dll.log");
        let (diagnosis, matched) = diagnose_match(text, None).unwrap();
        assert_eq!(diagnosis.title, "The Java runtime is broken");
        assert!(matched.contains("management.dll: The specified procedure could not be found"));
        assert!(diagnosis.mods.is_empty());
        assert!(!diagnosis.repair);
        let diagnosis = crate::crash_conflicts::refine(None, text, &[]).unwrap();
        assert_eq!(diagnosis.title, "The Java runtime is broken");
        let report = Report::build(
            "26.2",
            "Fabric",
            "A mod is for a different version",
            &[],
            &[],
            Some(text),
            None,
        );
        assert_eq!(report.title, "The Java runtime is broken");
    }

    #[test]
    fn runtime_native_errors_require_a_managed_library_path() {
        for line in [
            r"C:\Arctic Data\runtimes\java-runtime-epsilon\bin\server\jvm.dll: Can't find dependent libraries",
            r"C:\Arctic Data\runtimes\java-runtime-epsilon\bin\management.dll: The specified procedure could not be found",
            "java.lang.UnsatisfiedLinkError: /home/user/Arctic Data/runtimes/java-runtime-epsilon/lib/libmanagement.so: undefined symbol",
        ] {
            assert!(runtime_native_error(line).is_some(), "{line}");
        }
        for line in [
            r"java.lang.UnsatisfiedLinkError: C:\Windows\System32\ucrtbase.dll: The specified procedure could not be found",
            "java.lang.UnsatisfiedLinkError: /usr/lib/jvm/java-25/lib/libmanagement.so: undefined symbol",
            "java.lang.UnsatisfiedLinkError: /home/user/runtimes/java-runtime-epsilon/lib/library.jar",
        ] {
            assert!(runtime_native_error(line).is_none(), "{line}");
        }
    }

    #[test]
    fn first_error_prefers_crash_report_over_pack_metadata_warnings() {
        let log = include_str!("../tests/fixtures/crash-325-pack-warning.log");
        let report = include_str!("../tests/fixtures/crash-330-main-exception.log");
        let expected = "Caused by: java.lang.RuntimeException: underlying failure";
        assert_eq!(first_java_error(log, Some(report)), Some(expected));
        let uploaded = Report::build("1.21", "fabric", "unknown", &[log], &[], Some(report), None);
        assert!(
            uploaded
                .log
                .starts_with(&format!("\n---- first error ----\n{expected}\n"))
        );
        assert!(uploaded.log.contains("pack.mcmeta"));
        assert_eq!(
            first_java_error(
                &format!(
                    "{log}\n[Render thread/ERROR] Uncaught exception\njava.lang.IllegalStateException: Failed to initialize renderer\n\tat example.Game.render(Game.java:42)"
                ),
                None
            ),
            Some("java.lang.IllegalStateException: Failed to initialize renderer")
        );
        let earlier = format!(
            "[main/ERROR] java.lang.RuntimeException: handled\n\tat example.Optional.load(Optional.java:1)\n{log}"
        );
        assert_eq!(first_java_error(&earlier, Some(report)), Some(expected));
        assert_eq!(
            first_java_error("java.lang.Exception: no stack", None),
            None
        );
        assert_eq!(
            first_java_error(
                "java.lang.Error: fatal\n\tat example.Game.run(Game.java:1)",
                None
            ),
            Some("java.lang.Error: fatal")
        );
        assert_eq!(first_java_error(log, None), None);
        assert_eq!(
            first_java_error(
                log,
                Some("---- Minecraft Crash Report ----\nDescription: Unknown")
            ),
            None
        );
    }

    #[test]
    fn first_error_prefers_intel_vulkan_crash_over_public_keys_noise() {
        let log = "[19:10:32] [Render thread/ERROR]: Failed to request https://api.minecraftservices.com/publickeys
com.mojang.authlib.exceptions.MinecraftClientException: Failed to read from https://api.minecraftservices.com/publickeys due to Read timed out
    at knot//com.mojang.authlib.yggdrasil.YggdrasilServicesKeyInfo.fetch(YggdrasilServicesKeyInfo.java:114)
Caused by: java.net.SocketTimeoutException: Read timed out
    at java.base/sun.nio.ch.NioSocketImpl.timedRead(NioSocketImpl.java:278)";
        assert!(first_java_error(log, None).is_none());
        let (_, clipped) = log.split_once('\n').unwrap();
        assert!(first_java_error(clipped, None).is_none());
        assert!(first_exceptions(log).is_empty());

        let hs_err = "# A fatal error has been detected by the Java Runtime Environment:
#
#  EXCEPTION_ACCESS_VIOLATION (0xc0000005) at pc=0x00007ffb86ea5e87, pid=13344, tid=17184
#
# Problematic frame:
# C  [igvk64.dll+0x2a5e87]
#
Native frames: (J=compiled Java code, j=interpreted, Vv=VM code, C=native code)
C  [igvk64.dll+0x2a5e87]
j  org.lwjgl.vulkan.VK10.nvkCmdDrawIndexed(JIIIII)V+0
J  7821 c2 net.vulkanmod.vulkan.Renderer.drawIndexed(I)V (74 bytes) @ 0x000001f9367b28c0
";
        let fatal = hs_err.lines().nth(2).unwrap();
        let earlier_error = "[Render thread/ERROR]: An optional renderer failed\njava.lang.IllegalStateException: renderer failed\n    at example.Renderer.init(Renderer.java:42)";
        for (lines, attachment) in [
            ([log, earlier_error, fatal], Some(hs_err)),
            ([log, earlier_error, hs_err], None),
        ] {
            let report = Report::build(
                "1.21.11",
                "fabric",
                "unknown",
                &lines,
                &[],
                attachment,
                None,
            );
            assert!(report.log.starts_with(
                "\n---- first error ----\n#  EXCEPTION_ACCESS_VIOLATION (0xc0000005) in igvk64.dll\nJava frame: net.vulkanmod.vulkan.Renderer.drawIndexed\n"
            ));
        }
        assert_eq!(
            native_first_error("# Problematic frame:\n# C  [igvk64.dll+0x2a5e87]").as_deref(),
            Some("Error: Native crash in igvk64.dll")
        );
        assert_eq!(
            native_first_error(fatal).as_deref(),
            Some("#  EXCEPTION_ACCESS_VIOLATION (0xc0000005)")
        );
    }

    #[test]
    fn first_error_skips_background_auth_and_uses_the_deepest_report_cause() {
        let fixture = include_str!("../tests/fixtures/crash-451-unassigned-entity.log");
        let (startup, _) = fixture.split_once("---- logs/arctic-freezes/").unwrap();
        let startup = format!(
            "{startup}[Realms Availability/ERROR] Request failed\n\
            java.io.IOException: HTTP status 401\n\tat example.Realms.request(Realms.java:1)"
        );
        assert_eq!(first_java_error(&startup, None), None);
        assert!(first_exceptions(&startup).is_empty());
        let uncaught = "Exception in thread \"Download-3\" java.io.IOException: HTTP 401\n\tat example.Auth.fetch(Auth.java:1)";
        assert_eq!(first_java_error(uncaught, None), None);

        let fatal = "java.lang.IllegalStateException: renderer failed";
        let log = format!(
            "{startup}\nException in thread \"Render thread\" {fatal}\n\tat example.Game.render(Game.java:1)"
        );
        assert_eq!(first_java_error(&log, None), Some(fatal));

        let cause = "Caused by: java.lang.IllegalArgumentException: invalid renderer state";
        let report = format!(
            "---- Minecraft Crash Report ----\nDescription: Rendering entity in world\n\n\
            {fatal}\n\tat example.Game.render(Game.java:1)\n\
            Caused by: java.lang.RuntimeException: setup failed\n\tat example.Game.setup(Game.java:2)\n\
            {cause}\n\tat example.Game.init(Game.java:3)\n\
            \tSuppressed: java.lang.RuntimeException: cleanup failed\n\tCaused by: java.lang.IllegalStateException: cleanup\n"
        );
        assert_eq!(first_java_error(&log, Some(&report)), Some(cause));
        assert_eq!(
            first_java_error(&format!("{log}\n{report}"), None),
            Some(cause)
        );

        let download = "java.io.IOException: disk full";
        let log = format!(
            "{startup}\n[Download-2/ERROR] Failed to save assets\n{download}\n\tat example.Assets.write(Assets.java:1)"
        );
        assert_eq!(first_java_error(&log, None), Some(download));
    }

    use super::*;

    const REPORT_314: &str = include_str!("../tests/fixtures/crash-314-registry-sync.log");

    const QUILT_DUPLICATE_355: &str = "Duplicate mandatory mod ids [fabric-api, fabric] - <mods>/fabric-api-0.141.3+1.21.11.jar - <mods>/fabric-api-0.141.6+1.21.11.jar";
    const QUILT_LOADER_355: &str = "2 mods requires version [0.19.5, ∞) of fabricloader, which is missing! - <mods>/arctic-mod.jar - <mods>/fabric-language-kotlin-1.14.2+kotlin.2.4.21.jar";

    #[test]
    fn quilt_355_names_duplicate_jars_without_an_installed_mod_inventory() {
        let d = diagnose(QUILT_DUPLICATE_355).unwrap();
        assert_eq!(d.title, "Duplicate mod: fabric-api installed twice");
        assert_eq!(d.mods, ["fabric-api", "fabric"]);
        for file in [
            "fabric-api-0.141.3+1.21.11.jar",
            "fabric-api-0.141.6+1.21.11.jar",
        ] {
            assert!(d.detail.contains(file));
        }
        assert!(d.detail.contains("Remove one"));
    }

    #[test]
    fn quilt_355_names_mods_requiring_a_newer_fabric_compatibility_version() {
        let d = diagnose(QUILT_LOADER_355).unwrap();
        assert_eq!(d.title, "Fabric Loader is too old for these mods");
        assert_eq!(
            d.mods,
            [
                "arctic-mod.jar",
                "fabric-language-kotlin-1.14.2+kotlin.2.4.21.jar"
            ]
        );
        assert!(d.detail.contains("[0.19.5, ∞)"));
        assert!(d.detail.contains("Update Quilt Loader"));
        // The existing one-click loader update only supports Fabric instances.
        assert!(d.fix.is_none());
    }

    #[test]
    fn quilt_355_first_error_outranks_system_exit_in_logs_and_crash_reports() {
        for (first, second) in [
            (QUILT_DUPLICATE_355, QUILT_LOADER_355),
            (QUILT_LOADER_355, QUILT_DUPLICATE_355),
        ] {
            let expected = diagnose(first).unwrap().title;
            let text = format!(
                "---- Quilt Loader: Failed to load ----\n-- Error 1 --\n{first}\n-- Error 2 --\n{second}\n[Arctic diagnostics] exit caller other org.quiltmc.loader.impl.QuiltLoaderImpl.runPlugins\nexit code 1 (0x00000001)"
            );
            for text in [
                text.clone(),
                text.replace(" - ", "\n\n- "),
                text.replace(" - ", "\r\n\r\n- "),
            ] {
                assert_eq!(diagnose(&text).unwrap().title, expected);
                assert_eq!(diagnose(&text).unwrap().mods, diagnose(first).unwrap().mods);
                for crash_report in [None, Some(text.as_str())] {
                    let report = Report::build(
                        "1.21.11",
                        "Quilt 0.30.0-beta.3",
                        "Game exited with code 1 (0x00000001)",
                        &[&text],
                        &[],
                        crash_report,
                        None,
                    );
                    assert_eq!(report.title, expected);
                    assert!(report.log.contains("---- first error ----"));
                    assert!(
                        report
                            .log
                            .contains(&format!("---- diagnosis ----\n{expected}"))
                    );
                }
            }
        }
    }

    #[test]
    fn report_314_diagnoses_registry_sync_without_guessing_a_mod() {
        for report in [REPORT_314.to_owned(), REPORT_314.replace('\n', "\r\n")] {
            let (d, matched) = diagnose_match(&report, None).unwrap();
            assert_eq!(d.title, "Forge registry IDs did not sync");
            assert!(d.detail.contains("expected IDs"));
            assert!(d.detail.contains("mod conflict in the pack"));
            assert!(d.detail.contains("pack author"));
            assert!(d.mods.is_empty());
            assert!(!d.repair);
            assert!(d.fix.is_none());
            assert!(matched.starts_with("java.lang.RuntimeException:"));
            assert!(matched.contains("did not copy to the correct id"));
            assert!(!matched.contains("minVersion"));
        }
        for message in [
            "One or more entry values did not copy to the correct ID. Check log for details!",
            "Registry ID mismatch while syncing",
            "Mismatched registry IDs",
            "Registry synchronization failed",
            "Registry IDs do not match",
        ] {
            let changed = REPORT_314.replace(
                "One of more entry values did not copy to the correct id. Check log for details!",
                message,
            );
            assert_eq!(
                diagnose(&changed).unwrap().title,
                "Forge registry IDs did not sync"
            );
        }
        let other = REPORT_314.replace("ForgeRegistry.sync(", "AnotherClass.sync(");
        assert!(registry_sync_diagnosis(&other).is_none());
    }

    #[test]
    fn registry_diagnosis_names_only_entries_from_explicit_registry_errors() {
        let errors = "[Render thread/WARN] [net.minecraftforge.registries.ForgeRegistry/REGISTRIES]: Registry minecraft:item: Object did not get ID it asked for. Name: example:widget Expected: 24 Got: 25\n\
            [Render thread/ERROR] [net.minecraftforge.registries.ForgeRegistry/REGISTRIES]: Registry minecraft:block: Override did not have an associated owner object. Name: another:stone Value: example.Object\n\
            [Render thread/WARN] [net.minecraftforge.registries.ForgeRegistry/REGISTRIES]: Registry custom:registry: Object did not get ID it asked for. Name: minecraft:stone Expected: 1 Got: 2\n\
            [main/INFO] Registry minecraft:item: Object did not get ID it asked for. Name: ignored:entry Expected: 1 Got: 2\n\
            [main/ERROR] An unrelated mod error. Name: unrelated:entry\n";
        let d = diagnose(&format!("{errors}{errors}\n{REPORT_314}")).unwrap();
        assert_eq!(d.mods, ["example", "another"]);
        assert!(d.detail.contains("Another mod may have replaced"));
        assert!(d.fix.is_none());
        assert!(!d.detail.contains("modernfix"));
        assert!(!d.detail.contains("alltheleaks"));
    }

    #[test]
    fn registry_evidence_survives_noise_and_upload_limits_and_is_scrubbed() {
        let error = "[Render thread/WARN] [net.minecraftforge.registries.ForgeRegistry/REGISTRIES]: Registry minecraft:item: Object did not get ID it asked for. Name: example:widget Expected: 24 Got: 25 contact user@example.com";
        let noise = "noise\n".repeat(4000);
        let verbose = "verbose".repeat(4000);
        let report = Report::build(
            "1.20.1",
            "Forge",
            "alltheleaks failed to change the game",
            &[&noise, error, error, &noise],
            &[&verbose],
            Some(REPORT_314),
            Some(&verbose),
        );
        assert_eq!(report.title, "Forge registry IDs did not sync");
        assert!(report.log.contains("---- registry failure context ----"));
        assert_eq!(report.log.matches("Name: example:widget").count(), 1);
        assert!(report.log.contains("Expected: 24 Got: 25"));
        assert!(!report.log.contains("user@example.com"));
        assert!(report.log.contains("matched: java.lang.RuntimeException:"));
        assert!(report.log.len() < 48 * 1024);

        let many = (0..200).map(|id| format!(
            "[Render thread/ERROR] Registry minecraft:item ID mismatch Name: example:item_{id} Expected: 1 Got: 2 {}\n", "é".repeat(40)
        )).collect::<String>();
        let evidence = registry_failure_evidence(&many);
        assert!(evidence.len() <= REPORT_REGISTRY_BYTES);
        assert!(evidence.contains("example:item_199"));
        assert!(!evidence.contains("example:item_0 "));
        assert!(evidence.ends_with('\n'));
        let long = format!("{error}{}", "é".repeat(10_000));
        assert!(registry_failure_evidence(&long).len() <= REPORT_REGISTRY_BYTES);
        assert!(
            registry_failure_evidence("[main/INFO] Registry IDs remapped successfully").is_empty()
        );
    }

    #[test]
    fn mixin_matched_line_is_the_failure_not_a_config_warning() {
        let warning = REPORT_314.lines().next().unwrap();
        let failure =
            "InjectionError: Critical injection failure in realaddon.mixins.json:AddonMixin";
        let (_, matched) = diagnose_match(&format!("{warning}\n{failure}"), None).unwrap();
        assert_eq!(matched, failure);
    }

    #[test]
    fn oneconfig_download_failure_317() {
        let text = include_str!("../tests/fixtures/crash-317-oneconfig.log");
        let (d, matched) = diagnose_match(text, None).unwrap();
        assert_eq!(d.title, "Patcher couldn't download OneConfig (network)");
        assert!(d.detail.contains("Arctic will retry"));
        assert!(matched.contains("failed to download"));
        assert!(diagnose("OneConfig loaded successfully\nexit code 1").is_none());
        assert!(diagnose("Unrelated mod failed to download texture").is_none());
    }

    #[test]
    fn built_report_carries_installed_and_free_memory() {
        let noise = "noise\n".repeat(4000);
        let report = Report::build("1.21.1", "Fabric", "unknown", &[&noise], &[], None, None);
        assert!(report.log.contains("---- memory ----\n"));
        if let (Some(total), Some(free)) = (report.ram_total_mb, report.ram_free_mb) {
            assert!(
                report
                    .log
                    .contains(&format!("{total} MB total, {free} MB free\n"))
            );
            assert!(free <= total);
        }
        assert!(report.log.len() <= 48 * 1024);
    }

    #[test]
    fn memory_section_marks_unreadable_values() {
        assert_eq!(
            memory_section(Some(16384), Some(4096)),
            "\n---- memory ----\n16384 MB total, 4096 MB free\n"
        );
        assert_eq!(
            memory_section(None, None),
            "\n---- memory ----\nunknown total, unknown free\n"
        );
    }

    #[test]
    fn logged_linkage_error_does_not_mask_memory_hang() {
        let text = "[main/WARN] Optional integration failed\njava.lang.NoSuchMethodError: example.optional()\n[Render thread/INFO] Starting JEI took 21.01 seconds\n---- logs/arctic-freezes/freeze-now.txt ----\nduration: 20.34s (still frozen when written)\nheap: 3868 MB used of 4100 MB max at start\nHang at exit: client hung; no recovery recorded.\nmemory setting 5120 MB is more than this computer can spare: using 4100 MB";
        let d = diagnose(text).unwrap();
        assert_eq!(d.title, "Minecraft ran out of memory and froze");
        assert!(d.detail.contains("capped"));
        assert!(d.detail.contains("fewer or lighter mods"));
        let report = Report::build("1.21.1", "Fabric", &d.title, &[text], &[], None, None);
        assert!(report.log.contains("matched: heap: 3868 MB"));
    }

    #[test]
    fn hang_signals_require_current_evidence_and_detect_gc_pressure() {
        let gc = "---- logs/arctic-freezes/freeze-now.txt ----\nduration: 9.97s\ngc during freeze: 16 collections, 1562 ms\n(exit code -805306369 (0xCFFFFFFF))";
        assert_eq!(
            diagnose(gc).unwrap().title,
            "Minecraft ran out of memory and froze"
        );
        assert!(diagnose("exit code -805306369 (0xCFFFFFFF)").is_none());
        assert!(diagnose("---- logs/arctic-freezes/freeze-old.txt ----\nduration: 3.0s\nheap: 4000 MB used of 4100 MB max at start").is_none());
        let recovered = "---- logs/arctic-freezes/freeze-old.txt ----\nduration: 3.0s (still frozen when written)\n---- logs/arctic-freezes/freeze-new.txt ----\nduration: 4.0s";
        assert!(diagnose(recovered).is_none());
        let fatal = "[Render thread/ERROR] Uncaught exception in thread main\njava.lang.IllegalStateException: broken\n---- logs/arctic-freezes/freeze-now.txt ----\nduration: 20.34s (still frozen when written)";
        assert!(diagnose(fatal).is_none());
    }

    #[test]
    fn missing_recommended_dependency_crash_387() {
        let log = include_str!("../tests/fixtures/crash-387-music-hud.log");
        let (d, matched) = diagnose_match(log, None).unwrap();
        assert_eq!(d.title, "Music HUD needs Modern UI");
        assert_eq!(d.detail, "Install Modern UI or remove Music HUD.");
        assert_eq!(d.mods, ["Music HUD", "Modern UI"]);
        assert!(matched.contains("NoClassDefFoundError: icyllis/modernui/"));
    }

    #[test]
    fn unrelated_recommendations_keep_the_version_diagnosis() {
        let log = include_str!("../tests/fixtures/crash-387-music-hud.log");
        let (warnings, crash) = log.split_once("---- Minecraft Crash Report ----").unwrap();
        assert!(diagnose(warnings).is_none());
        let text = format!(
            "{}\n---- Minecraft Crash Report ----{crash}",
            warnings
                .lines()
                .filter(|line| !line.contains("modernui"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert_eq!(
            diagnose(&text).unwrap().title,
            "A mod is for a different version"
        );
    }

    #[test]
    fn suggested_dependency_can_use_a_package_alias() {
        for (warning, class, name) in [
            (
                "any version of fabric-language-kotlin",
                "kotlin.jvm.internal.Intrinsics",
                "Fabric Language Kotlin",
            ),
            (
                "'Cloth Config' (cloth-config)",
                "me.shedaniel.clothconfig.api.ConfigBuilder",
                "Cloth Config",
            ),
        ] {
            let log = format!(
                "- Mod 'Other Mod' (other_mod) 1.0 suggests {warning}, which is missing!\n\
                 - Mod 'Example' (example) 1.0 suggests {warning}, which is missing!\n\
                 ---- Minecraft Crash Report ----\n\
                 java.lang.RuntimeException: Could not execute entrypoint stage 'client' due to errors, provided by 'example' at 'example.Client'!\n\
                 Caused by: java.lang.ClassNotFoundException: {class}"
            );
            assert_eq!(
                diagnose(&log).unwrap().title,
                format!("Example needs {name}")
            );
        }
    }

    #[test]
    fn fatal_linkage_errors_keep_version_diagnosis_and_trigger() {
        for text in [
            "[main/INFO] Starting\nException in thread \"main\" java.lang.NoSuchMethodError: example.missing()",
            "[main/ERROR] Uncaught exception in thread \"main\"\njava.lang.NoSuchMethodError: example.missing()",
            "---- Minecraft Crash Report ----\njava.lang.NoSuchMethodError: example.missing()",
            "[Render thread/FATAL] Encountered an unexpected exception\njava.lang.NoSuchMethodError: example.missing()",
        ] {
            let d = diagnose(text).unwrap();
            assert_eq!(d.title, "A mod is for a different version");
            let noise = "\tat example.LongStack.frame(LongStack.java:1)\n".repeat(300);
            let report = Report::build("1.21.1", "", &d.title, &[text, &noise], &[], None, None);
            assert!(report.log.contains("matched: "));
            assert!(
                report
                    .log
                    .contains("java.lang.NoSuchMethodError: example.missing()")
            );
        }
        assert!(
            diagnose(
                "[main/ERROR] Caught java.lang.NoSuchMethodError: optional\n[main/INFO] Loaded"
            )
            .is_none()
        );
        assert!(
            diagnose("[main/WARN] java.lang.NoClassDefFoundError: org/lwjgl/Optional").is_none()
        );
    }

    const CLASS_PROBE_522: &str = "[16:29:43] [main/WARN]: Error loading class: com/firemerald/additionalplacements/common/ConfigCommon (java.lang.ClassNotFoundException: com.firemerald.additionalplacements.common.ConfigCommon)";
    const PROVIDER_ERROR_522: &str = "net.minecraftforge.fml.ModLoadingException: Mod File Resourcify (1.20.1-forge)-1.8.7.jar needs language provider kotlinforforge:4.11 or above to load";
    const FOLLOW_ON_522: &str = r#"---- Minecraft Crash Report ----
Description: Initializing game

java.lang.ExceptionInInitializerError: null
    at net.minecraft.client.gui.Gui.<clinit>(Gui.java:81)
Caused by: java.lang.NullPointerException: Cannot invoke "net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext.getModEventBus()" because the return value of "net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext.get()" is null
    at com.tterrag.registrate.AbstractRegistrate.getModEventBus(AbstractRegistrate.java:222)
    at com.petrolpark.destroy.Destroy.<clinit>(Destroy.java:97)"#;

    #[test]
    fn crash_522_prefers_the_language_provider_error() {
        let log = format!(
            "{CLASS_PROBE_522}\n\
             [16:29:50] [Render thread/ERROR] Error during pre-loading phase\n    \
             {PROVIDER_ERROR_522}\n    §7We have found 4.10.0\n\
             [16:29:50] [Render thread/ERROR]: Cowardly refusing to send event net.minecraftforge.client.event.sound.SoundEngineLoadEvent to a broken mod state"
        );
        let text = format!("{log}\n{FOLLOW_ON_522}");
        let title = "Resourcify (1.20.1-forge)-1.8.7.jar needs Kotlin for Forge 4.11 or newer";
        let (diagnosis, matched) = diagnose_match(&text, None).unwrap();
        assert_eq!(diagnosis.title, title);
        assert!(
            diagnosis
                .detail
                .contains("Install or update Kotlin for Forge")
        );
        assert!(!diagnosis.repair);
        assert!(diagnosis.fix.is_none());
        assert_eq!(matched.trim(), PROVIDER_ERROR_522);
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            crate::crash_conflicts::diagnose_exit_in(&text, -1, dir.path()),
            Some(diagnosis)
        );
        let report = Report::build(
            "1.20.1",
            "Forge",
            "A mod is for a different version",
            &log.lines().collect::<Vec<_>>(),
            &[],
            Some(FOLLOW_ON_522),
            None,
        );
        assert_eq!(report.title, title);
        assert!(
            report
                .log
                .contains(&format!("{title}\nmatched: {PROVIDER_ERROR_522}"))
        );
    }

    #[test]
    fn crash_522_class_probe_does_not_supply_a_diagnosis() {
        for warning in [
            CLASS_PROBE_522.to_owned(),
            CLASS_PROBE_522.replace("[main/WARN]", "[WARNING]"),
            CLASS_PROBE_522.replace(
                "com.firemerald.additionalplacements.common.ConfigCommon",
                "cpw.mods.bootstraplauncher.BootstrapLauncher",
            ),
        ] {
            assert!(diagnose(&warning).is_none(), "{warning}");
            let text = format!("{FOLLOW_ON_522}\n\n{warning}");
            assert!(diagnose_match(&text, None).is_none(), "{warning}");
        }
    }

    #[test]
    fn early_forge_dependency_errors_outrank_follow_on_linkage_errors() {
        let linkage =
            "---- Minecraft Crash Report ----\njava.lang.NoSuchMethodError: example.missing()";
        assert_eq!(
            diagnose(linkage).unwrap().title,
            "A mod is for a different version"
        );
        let mandatory = "Missing or unsupported mandatory dependencies";
        let row = "Mod ID: 'jei', Requested by: 'example', Expected range: '[15,)', Actual version: '[MISSING]'";
        for (error, title, matched) in [
            (
                r"net.neoforged.fml.ModLoadingException: Loading errors encountered:
    - Mod File mods\Example Mod.jar needs language provider example_fml:2.3-beta or above to load",
                "Example Mod.jar needs example_fml 2.3-beta or newer",
                "Mod File",
            ),
            (
                "Failure message: Mod File mods/example.jar needs language provider example_fml to load",
                "example.jar needs example_fml",
                "Mod File",
            ),
            (
                "    net.minecraftforge.fml.ModLoadingException: Missing or unsupported mandatory dependencies",
                "A required mod dependency is missing or incompatible",
                mandatory,
            ),
            (
                &format!(
                    "[main/ERROR] [net.minecraftforge.fml.loading.ModSorter/LOADING]: {mandatory}:\n    {row}"
                ),
                "A mod needs jei",
                row,
            ),
        ] {
            let text = format!("{error}\n{linkage}");
            let (d, evidence) = diagnose_match(&text, None).unwrap();
            assert_eq!(d.title, title);
            assert!(evidence.contains(matched), "{evidence}");
        }
    }

    #[test]
    #[ignore = "requires offline client logs from the diagnostics proof runs"]
    fn real_client_diagnostic_reports() {
        let dir = std::path::PathBuf::from(std::env::var_os("CRASHDIAG_PROOF_DIR").unwrap());
        for (version, mode) in [
            ("1.8.9", "exit77"),
            ("1.20.1", "exit77"),
            ("26.3", "exit77"),
            ("26.3", "sleep40"),
        ] {
            let log = std::fs::read_to_string(dir.join(format!("{version}-{mode}.log"))).unwrap();
            let mut parser = crate::launch::logparse::LogParser::default();
            let plain = log
                .lines()
                .flat_map(|line| parser.feed(line))
                .map(|l| l.text)
                .collect::<Vec<_>>();
            let lines = plain.iter().map(String::as_str).collect::<Vec<_>>();
            let report = Report::build(
                version,
                "fabric",
                "",
                &lines,
                &[],
                None,
                Some("exit code 77 (0x0000004D)"),
            );
            if mode == "sleep40" {
                assert_eq!(report.title, "The game froze and was closed");
                assert!(
                    report.log.contains("java.lang.Thread.sleep")
                        || report
                            .log
                            .contains("java.base@25.0.1/java.lang.Thread.sleep")
                );
            } else {
                assert!(
                    report.title.contains("diagnosticsExit77"),
                    "{}",
                    report.title
                );
                assert!(report.log.contains("exit caller other"));
            }
            assert!(report.log.len() < 48 * 1024);
            std::fs::write(
                dir.join(format!("{version}-{mode}-report.txt")),
                format!("{}\n{}", report.title, report.log),
            )
            .unwrap();
        }
    }

    #[test]
    fn freeze_classification_needs_evidence_and_silent_titles_split_by_caller() {
        let hang = "[Arctic diagnostics] hang begin\n[Arctic diagnostics] hang end";
        let recovered = format!(
            "{hang}\n[Arctic diagnostics] hang recovered; render progress resumed\nexit code 77"
        );
        assert!(freeze_evidence(hang));
        assert!(!freeze_evidence(&recovered));
        let report = Report::build(
            "1.20.1",
            "",
            "",
            &[&recovered],
            &[],
            None,
            Some("exit code 77"),
        );
        assert_ne!(report.title, "The game froze and was closed");
        assert!(report.log.contains("hang recovered"));
        assert!(freeze_evidence(&format!("{recovered}\n{hang}")));
        assert!(!freeze_evidence("exit code -805306369 (0xCFFFFFFF)"));
        assert!(!freeze_evidence("Can't keep up! 160523ms behind"));
        assert!(!freeze_evidence(
            "Can't keep up! 160523ms behind\nexit code 1 (0x00000001)"
        ));
        for prefix in ["Can't keep up! 60000ms behind", hang, &recovered] {
            let text = format!(
                "{prefix}\njava.lang.OutOfMemoryError: Java heap space\nexit code 1 (0x00000001)"
            );
            let diagnosis = diagnose(&text).expect("specific memory diagnosis");
            let report = Report::build("1.20.1", "", &diagnosis.title, &[&text], &[], None, None);
            assert_eq!(report.title, diagnosis.title);
        }
        let report = Report::build(
            "1.20.1",
            "",
            "",
            &["[Arctic diagnostics] exit caller other mod.Example.quit(Example.java:12)"],
            &[],
            None,
            Some("exit code 77"),
        );
        assert_eq!(
            report.title,
            "Game exited with code 77: mod.Example.quit(Example.java:12)"
        );
    }

    #[test]
    fn diagnostics_survive_tail_and_keep_report_under_limit() {
        let mut lines = vec![
            "[Arctic diagnostics] hang begin",
            "Render thread sleeping",
            "[Arctic diagnostics] hang end",
            "Missing metadata in pack broken.zip",
        ];
        lines.extend(std::iter::repeat_n("later noise", 400));
        let command = format!("launching: java -cp \"{}\" Main", "huge.jar:".repeat(3000));
        let launcher = ["important launcher evidence", command.as_str()];
        let report = Report::build(
            "26.3",
            "fabric",
            "unknown",
            &lines,
            &launcher,
            None,
            Some("java summary"),
        );
        assert_eq!(report.title, "The game froze and was closed");
        assert!(report.log.contains("Render thread sleeping"));
        assert!(report.log.contains("Broken resource-pack metadata:"));
        assert!(report.log.contains("important launcher evidence"));
        assert!(report.log.contains("<classpath>"));
        assert!(report.log.len() < 48 * 1024);
    }

    #[test]
    fn diagnostic_sections_stay_scrubbed_and_bounded() {
        let own = Own::this_pc();
        let home = own.home.as_deref().unwrap_or("/home/example");
        let diagnostic = format!(
            "[Arctic diagnostics] exit caller other example.Exit.quit(Exit.java:1) {home} --accessToken secret-token --username SecretName"
        );
        let noise = "noise ".repeat(10_000);
        let dump =
            format!("[Arctic diagnostics] hang begin\n{noise}\n[Arctic diagnostics] hang end");
        let report = Report::build(
            "26.3",
            "",
            "",
            &[&diagnostic, &dump],
            &[&noise],
            Some(&noise),
            Some(&noise),
        );
        assert!(report.log.len() < 48 * 1024);
        assert!(!report.log.contains("secret-token"));
        assert!(!report.log.contains("SecretName"));
        if own.home.is_some() {
            assert!(!report.log.contains(home));
        }
        assert!(!report.title.contains(home));
    }

    #[test]
    fn shutdown_requires_caller_and_completed_stop() {
        let text = "Stopping!\nAll dimensions are saved\n[Arctic diagnostics] minecraft stop complete\n[Arctic diagnostics] exit caller vanilla net.minecraft.client.Minecraft.destroy";
        assert!(orderly_shutdown(text));
        assert!(!orderly_shutdown("Stopping!\nSaved all chunks"));
        assert!(!orderly_shutdown(&format!(
            "{text}\n[Arctic diagnostics] exit caller other mod.Exit.quit"
        )));
        assert!(exit_description(-805306369).contains("0xCFFFFFFF"));
        let report = Report::build("1.20.1", "", "unknown", &[text], &[], None, None);
        assert!(report.title.starts_with("Non-crash:"));
    }

    #[test]
    fn normal_shutdown_preserves_specific_crash_diagnosis() {
        let text = "java.lang.OutOfMemoryError: Java heap space\nStopping!\nAll dimensions are saved\n[Arctic diagnostics] minecraft stop complete\n[Arctic diagnostics] exit caller vanilla net.minecraft.client.Minecraft.destroy\nexit code 1 (0x00000001)";
        assert!(orderly_shutdown(text));
        let diagnosis = diagnose(text).expect("specific memory diagnosis");
        let report = Report::build("1.20.1", "", &diagnosis.title, &[text], &[], None, None);
        assert_eq!(report.title, "Minecraft ran out of memory");
    }

    #[test]
    fn driver_diagnostics_match_the_closed_groups() {
        assert!(
            diagnose("SDL3.dll failed: error code = 1114")
                .unwrap()
                .detail
                .contains("High-performance GPU")
        );
        assert!(diagnose("BackendCreationException: OpenGL is not supported: Could not retrieve OpenGL functions").unwrap().detail.contains("XWayland"));
    }

    #[test]
    fn java_crash_files_are_cut_before_the_process_section() {
        let file = "# A fatal error
#  EXCEPTION_ACCESS_VIOLATION

---------------  S U M M A R Y ------------

Command Line: -Xmx2G --accessToken abc net.minecraft.client.main.Main

Host: AMD Ryzen

---------------  T H R E A D  ---------------

Stack: [0x1,0x2]
j  some.Frame()

---------------  P R O C E S S  ---------------

VM Arguments:
jvm_args: -Xmx2G

Environment Variables:
USERNAME=someone
";
        let cut = trim_hs_err(file);
        assert!(cut.contains("EXCEPTION_ACCESS_VIOLATION"));
        assert!(cut.contains("Host: AMD Ryzen"));
        assert!(cut.contains("j  some.Frame()"));
        assert!(!cut.contains("accessToken"));
        assert!(!cut.contains("Command Line"));
        assert!(!cut.contains("Environment"));
        assert!(!cut.contains("USERNAME"));
    }

    #[test]
    fn the_last_maximum_heap_flag_wins_in_a_java_summary() {
        let java = "Java javaw.exe, flags: -Xms512M -Xmx6144M -Xmx3584M -XX:+UseG1GC | classpath: client.jar";
        assert_eq!(max_memory_mb(java), Some(3584));
        assert_eq!(max_memory_mb("-Xmx6144M -Xmx4g"), Some(4096));
        assert_eq!(max_memory_mb("-Xmx6144M -Xmxbad"), None);
    }

    #[test]
    fn heap_exhaustion_explains_custom_flags_in_old_and_new_java_summaries() {
        let error = "java.lang.OutOfMemoryError: Java heap space";
        let summaries = [
            "Java javaw.exe, flags: -Xms512M -Xmx6144M -Xmx3584M -XX:+UseG1GC".to_owned(),
            format!("Java javaw.exe{CUSTOM_MAX_HEAP_SOURCE}, flags: -Xms512M -Xmx3584M"),
        ];
        for summary in summaries {
            let diagnosis = diagnose(&format!("{error}\n{summary}")).unwrap();
            assert_eq!(diagnosis.title, "Minecraft ran out of memory");
            assert!(diagnosis.detail.contains("heap to 3584 MB"));
            assert!(diagnosis.detail.contains("Remove the custom -Xmx"));
        }
        let default = diagnose(&format!("{error}\nJava javaw.exe, flags: -Xmx3584M")).unwrap();
        assert!(default.detail.contains("Settings → Memory"));
    }

    #[test]
    fn a_big_pack_with_little_memory_gets_a_warning_not_a_diagnosis() {
        assert_eq!(
            max_memory_mb("Java x, flags: -Xms512M -Xmx1918M -XX:+UseG1GC"),
            Some(1918)
        );
        assert_eq!(max_memory_mb("flags: -Xmx4G"), Some(4096));
        assert_eq!(max_memory_mb("flags: -Xms512M"), None);
        let warning = memory_warning(1918, 213, Some(3966)).unwrap();
        assert!(
            warning.contains("1.9 GB")
                && warning.contains("3.9 GB")
                && warning.contains("213 mod files")
        );
        // Enough memory, or a small pack: nothing to say.
        assert!(memory_warning(6144, 213, Some(16384)).is_none());
        assert!(memory_warning(1918, 20, Some(3966)).is_none());
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
    fn crash_517_names_the_damaged_library_and_requests_repair() {
        let log = include_str!("../tests/fixtures/crash-517-corrupt-library.log");
        let diagnosis =
            crate::crash_conflicts::diagnose_exit_in(log, 1, Path::new("mods")).unwrap();
        assert_eq!(diagnose(log), Some(diagnosis.clone()));
        assert_eq!(diagnosis.title, "A game file was damaged");
        assert_eq!(
            diagnosis.detail,
            "Minecraft couldn't read log4j-core-2.22.1.jar. Arctic will re-download it on the next launch."
        );
        assert!(diagnosis.repair);
    }

    #[test]
    fn damaged_jar_advice_depends_on_where_the_jar_is_installed() {
        let trace = |path, cause| {
            format!(
                "java.io.IOException: error reading {path}\n\tat loader.read(Loader.java:1)\nCaused by: java.util.zip.ZipException: {cause}"
            )
        };
        let version = trace(
            "/portable data/shared/versions/1.8.9/1.8.9.jar",
            "invalid LOC header (bad signature)",
        );
        let diagnosis = diagnose(&version).unwrap();
        assert!(diagnosis.detail.contains("1.8.9.jar"));
        assert!(diagnosis.repair);

        for path in [
            r"C:\game\profiles\p\instances\i\minecraft\mods\Example Mod.jar",
            "/custom game/mods/Example Mod.jar",
        ] {
            let report = trace(path, "zip file is empty");
            let diagnosis = crate::crash_conflicts::diagnose_exit_in(
                &report,
                1,
                Path::new("/custom game/mods"),
            )
            .unwrap();
            assert_eq!(diagnosis.title, "A game file was damaged");
            assert!(diagnosis.detail.contains("Example Mod.jar"));
            assert!(diagnosis.detail.contains("Re-download this mod"));
            assert!(!diagnosis.repair);
        }

        for report in [
            trace("/unrelated/libraries/example.jar", "zip END header not found"),
            trace("/data/shared/libraries/example.jar", "unrelated ZIP error"),
            "java.io.IOException: error reading /data/shared/libraries/example.jar\nOther failure\nCaused by: java.util.zip.ZipException: zip file is empty".into(),
        ] {
            assert!(damaged_jar_diagnosis(&report, None).is_none());
        }
    }

    #[test]
    fn missing_loader_entrypoints_request_repair_before_the_loader_starts() {
        for (class, loader) in [
            ("net.fabricmc.loader.launch.knot.KnotClient", "Fabric"),
            ("org.quiltmc.loader.impl.launch.knot.KnotClient", "Quilt"),
            (
                "cpw.mods.bootstraplauncher.BootstrapLauncher",
                "Forge/NeoForge",
            ),
            ("cpw.mods.modlauncher.Launcher", "Forge/NeoForge"),
            ("net.minecraft.launchwrapper.Launch", "Forge"),
        ] {
            for prefix in [
                "Error: Could not find or load main class",
                "java.lang.ClassNotFoundException:",
            ] {
                let diagnosis = diagnose(&format!("{prefix} {class}\n(exit code 1)")).unwrap();
                assert_eq!(
                    diagnosis.title,
                    format!("A game file was damaged (the {loader} loader)")
                );
                assert!(diagnosis.repair);
            }
        }
        // A malformed Java argument or a mod's missing class is not evidence of a damaged loader.
        assert!(diagnose("Error: Could not find or load main class Forge").is_none());
        assert!(diagnose("Error: Could not find or load main class com.example.Mod").is_none());
    }

    #[test]
    fn a_missing_loader_class_requests_a_library_check() {
        // Crash report #22: Fabric's Mixin bootstrap, before any mod ran.
        let log = "[main/ERROR] Uncaught exception in thread \"main\"
    java.lang.NoClassDefFoundError: org/spongepowered/asm/mixin/throwables/MixinError
    	at net.fabricmc.loader.impl.launch.FabricMixinBootstrap.init(FabricMixinBootstrap.java:59)
    Caused by: java.lang.ClassNotFoundException: org.spongepowered.asm.mixin.throwables.MixinError";
        let d = diagnose(log).unwrap();
        assert_eq!(d.title, "A game library couldn't be loaded");
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
    fn crash_378_names_entityculling_and_the_missing_fabric_api_module() {
        let exception = r"[main/ERROR]: Minecraft has crashed!
net.fabricmc.loader.impl.FormattedException: java.lang.RuntimeException: Could not execute entrypoint stage 'client' due to errors, provided by 'entityculling' at 'dev.tr7zw.entityculling.EntityCullingMod'!
Caused by: java.lang.NoClassDefFoundError: net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents
    at dev.tr7zw.entityculling.EntityCullingMod.initModloader(EntityCullingMod.java:23)
Caused by: java.lang.ClassNotFoundException: net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents";
        let log = format!(
            r#"launching Minecraft --gameDir "C:\Arctic Launcher\profiles\default\instances\vanilla\minecraft"
[main/INFO]: Loading Minecraft 1.16 with Fabric Loader 0.20.1
[main/INFO]: Loading 8 mods:
    - entityculling 1.5.2
    - fabric 0.13.1+build.370-1.16
       |-- fabric-api-base 0.1.3+12a8474c7c
       \-- fabric-key-binding-api-v1 1.0.0+e16a97747c
    - fabricloader 0.20.1
       \-- mixinextras 0.5.5
    - java 8
    - minecraft 1.16
{exception}"#
        );
        let (d, matched) = diagnose_match(&log, None).unwrap();
        assert_eq!(d.title, "EntityCulling needs Fabric API's lifecycle events");
        assert_eq!(d.mods, ["entityculling"]);
        assert!(d.detail.contains("fabric-lifecycle-events-v1"));
        assert!(d.detail.contains("missing or too old"));
        assert!(d.detail.contains("Arctic installed EntityCulling"));
        assert!(matched.contains("NoClassDefFoundError"));

        let other_instance = exception.replace(
            "java.lang.NoClassDefFoundError: net/fabricmc/fabric/api/client/event/lifecycle/v1/ClientTickEvents",
            "java.lang.ClassNotFoundException: net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents",
        );
        let d = diagnose(&other_instance).unwrap();
        assert_eq!(d.title, "EntityCulling needs Fabric API's lifecycle events");
        assert!(!d.detail.contains("Arctic installed"));
        assert!(
            diagnose("[main/WARN]: java.lang.ClassNotFoundException: net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents")
                .is_none()
        );
    }

    #[test]
    fn crash_488_points_quickthrow_at_its_missing_library() {
        let report = "---- Minecraft Crash Report ----\n\n\
            Description: Initializing game\n\n\
            java.lang.RuntimeException: Could not execute entrypoint stage 'client' due to errors, provided by 'quickthrow' at 'com.iamdeath.quickthrow.QuickThrowClient'!\n\
            \tat net.fabricmc.loader.impl.FabricLoaderImpl.invokeEntrypoints(FabricLoaderImpl.java:416)\n\
            Caused by: java.lang.NoClassDefFoundError: com/death/core/DeathCoreApi\n\
            \tat knot//com.iamdeath.quickthrow.QuickThrowClient.onInitializeClient(QuickThrowClient.java:42)\n\
            \t... 6 more\n\
            Caused by: java.lang.ClassNotFoundException: com.death.core.DeathCoreApi\n\
            \tat net.fabricmc.loader.impl.launch.knot.KnotClassLoader.loadClass(KnotClassLoader.java:119)\n\n\
            -- System Details --\n\
            \tFabric Mods:\n\
            \t\tfabric-api: Fabric API 0.161.0+26.2\n\
            \t\tquickthrow: QuickThrow 1.0.0\n";
        let (d, matched) = diagnose_match(report, None).unwrap();
        assert_eq!(d.title, "QuickThrow needs DeathCore");
        assert_eq!(d.mods, ["quickthrow"]);
        assert!(d.detail.contains("com.death.core.DeathCoreApi"));
        assert!(
            d.detail
                .contains("Install a compatible version of DeathCore")
        );
        assert!(d.detail.contains("remove QuickThrow"));
        assert!(!d.repair);
        assert!(matched.ends_with("NoClassDefFoundError: com/death/core/DeathCoreApi"));

        let dotted_only = report.replace(
            "Caused by: java.lang.NoClassDefFoundError: com/death/core/DeathCoreApi\n",
            "",
        );
        assert_eq!(diagnose(&dotted_only).unwrap(), d);
        for report in [
            report
                .replace("com/death/core/", "com/iamdeath/quickthrow/api/")
                .replace("com.death.core.", "com.iamdeath.quickthrow.api."),
            format!(
                "{}\t\tcloth-config: Cloth Config 1.0.0\n",
                report
                    .replace("com/death/core/DeathCoreApi", "me/shedaniel/clothconfig2/MissingClass")
                    .replace("com.death.core.DeathCoreApi", "me.shedaniel.clothconfig2.MissingClass"),
            ),
            format!("{report}\t\tdeath_core: DeathCore 1.0.0\n"),
            report.replace(
                "Caused by: java.lang.NoClassDefFoundError",
                "\njava.lang.RuntimeException: An unrelated failure\nCaused by: java.lang.NoClassDefFoundError",
            ),
        ] {
            assert_eq!(diagnose(&report).unwrap().title, "A mod is for a different version");
        }

        // A provider's jar can own the package even when its mod ID gives no clue.
        let mods = tempfile::tempdir().unwrap();
        let file = std::fs::File::create(mods.path().join("library.jar")).unwrap();
        let mut jar = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        jar.start_file("fabric.mod.json", options).unwrap();
        std::io::Write::write_all(
            &mut jar,
            br#"{"id":"library","name":"Library","version":"1.0.0"}"#,
        )
        .unwrap();
        jar.start_file("com/death/core/OtherClass.class", options)
            .unwrap();
        jar.finish().unwrap();
        assert_eq!(
            diagnose_with_java(report, None, mods.path(), &[])
                .unwrap()
                .title,
            "A mod is for a different version"
        );
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

    const CONNECTOR_RESOLUTION: &str = "[FabricLoader/Resolution]: Immediate reason: [HARD_DEP_NO_CANDIDATE betterend 4.0.11 {depends bclib @ [3.0.x]}, ROOT_FORCELOAD_SINGLE betterend 4.0.11]
[FabricLoader/Resolution]: Reason: [HARD_DEP betterend 4.0.11 {depends bclib @ [3.0.x]}]
[FabricLoader/Resolution]: Fix: add [], remove [], replace [[betterend 4.0.11] -> add:betterend 1 ([(-∞,∞)])]";

    // Report #279: Sodium breaks every VulkanMod version; Not Enough Vulkan's dependency is met.
    const VULKAN_SODIUM_CONFLICT: &str = "[main/INFO] Immediate reason: [HARD_DEP not-enough-vulkan 1.6.2+mc1.21.11 {depends vulkanmod @ [>=0.6.3]}, NEG_HARD_DEP sodium 0.8.14+mc1.21.11 {breaks vulkanmod @ [*]}, ROOT_FORCELOAD_SINGLE not-enough-vulkan 1.6.2+mc1.21.11, ROOT_FORCELOAD_SINGLE sodium 0.8.14+mc1.21.11]
[main/INFO] Reason: [NEG_HARD_DEP sodium 0.8.14+mc1.21.11 {breaks vulkanmod @ [*]}]
[main/INFO] Fix: add [], remove [], replace [[sodium 0.8.14+mc1.21.11] -> add:sodium 1 ([[0.8.7-,∞)])]
[main/ERROR] Incompatible mods found!
    net.fabricmc.loader.impl.FormattedException: Some of your mods are incompatible with the game or each other!
    A potential solution has been determined, this may resolve your problem:
    \t - Replace mod 'Sodium' (sodium) 0.8.14+mc1.21.11 with version 0.8.7- or later that is compatible with:
    \t\t - vulkanmod 0.6.8+1.21.11
    More details:
    \t - Mod 'Sodium' (sodium) 0.8.14+mc1.21.11 is incompatible with any version of mod 'VulkanMod' (vulkanmod), yet a conflicting version is present: 0.6.8+1.21.11!";

    #[test]
    fn sodium_and_vulkanmod_are_a_conflict_not_an_update() {
        let english = VULKAN_SODIUM_CONFLICT.split_once("[main/ERROR]").unwrap().1;
        let concrete_hint = VULKAN_SODIUM_CONFLICT.replace(
            "add:sodium 1 ([[0.8.7-,∞)])",
            "add:sodium 0.8.12 ([[0.8.12,∞)])",
        );
        for log in [VULKAN_SODIUM_CONFLICT, english, &concrete_hint] {
            let d = diagnose(log).unwrap();
            assert_eq!(d.title, "Sodium and VulkanMod don't work together");
            assert_eq!(
                d.detail,
                "Sodium doesn't work alongside VulkanMod. Remove one of them."
            );
            assert_eq!(d.mods, ["Sodium", "VulkanMod"]);
            assert!(!d.repair);
            assert_eq!(d.fix, None);
        }
    }

    #[test]
    fn resolver_breaks_override_dependency_and_replacement_hints_without_english() {
        let resolver = VULKAN_SODIUM_CONFLICT.split_once("[main/ERROR]").unwrap().0;
        let reason = resolver.lines().nth(1).unwrap();
        for log in [
            resolver.to_owned(),
            reason.to_owned(),
            format!(
                "{reason}\nFix: add [], remove [], replace [[sodium 0.8.14+mc1.21.11] -> add:sodium 0.8.12 ([[0.8.12,∞)])]"
            ),
        ] {
            let d = diagnose(&log).unwrap();
            assert_eq!(d.title, "sodium and vulkanmod don't work together");
            assert_eq!(
                d.detail,
                "sodium doesn't work alongside vulkanmod. Remove one of them."
            );
            assert_eq!(d.mods, ["sodium", "vulkanmod"]);
        }
    }

    #[test]
    fn a_satisfied_dependency_in_the_immediate_reason_is_not_missing() {
        let d = resolution_log(VULKAN_SODIUM_CONFLICT);
        assert!(d.title.is_empty());
        assert!(d.detail.is_empty());
        assert!(d.mods.is_empty());

        // Other failed constraints still matter; the satisfied dependency does not.
        let log = VULKAN_SODIUM_CONFLICT
            .split_once("[main/ERROR]")
            .unwrap()
            .0
            .replace(
                "Reason: [NEG_HARD_DEP sodium 0.8.14+mc1.21.11 {breaks vulkanmod @ [*]}]",
                "Reason: [HARD_DEP example 1.0 {depends fabric-api @ [*]}]",
            );
        let d = diagnose(&log).unwrap();
        assert_eq!(d.title, "A mod dependency is missing");
        assert_eq!(d.mods, ["example", "fabric-api"]);
        assert_eq!(
            d.detail,
            "example needs fabric-api (version [*]), which isn't installed."
        );
    }

    #[test]
    fn resolver_hints_ignore_single_numbers_with_a_different_specific_lower_bound() {
        assert_eq!(update_hint(VULKAN_SODIUM_CONFLICT), None);
        assert!(resolver_updates(VULKAN_SODIUM_CONFLICT).is_empty());
        for range in [
            "[[0.8.7-,∞)]",
            "[[0.8.7-,0.8.14]]",
            "[(-∞,0.8.14]]",
            "[[1.0,∞)]",
            "[[1-beta,∞)]",
        ] {
            let hint = format!("[[sodium 0.8.14] -> add:sodium 1 ({range})]");
            assert_eq!(update_hint(&hint), None, "{hint}");
        }
        // Filtering a placeholder must leave concrete updates in the same Fix line intact.
        let log = format!("{VULKAN_SODIUM_CONFLICT}\n[[other 0] -> add:other 1 ([[1,1]])]");
        assert_eq!(update_hint(&log).as_deref(), Some("update other to 1"));
    }

    #[test]
    fn split_packages_override_connector_resolver_hints() {
        // Report #272: Fabric mods running on Forge through Sinytra Connector.
        for fatal in [
            "Exception in thread \"main\" java.lang.module.ResolutionException: Modules luna_minecraft and e4all export package link.e4mc to module fabric_renderer_api_v1",
            "java.lang.module.ResolutionException: Module fabric_renderer_api_v1 reads package link.e4mc from both luna_minecraft and e4all",
        ] {
            for log in [fatal.to_owned(), format!("{CONNECTOR_RESOLUTION}\n{fatal}")] {
                let d = diagnose(&log).unwrap();
                assert_eq!(d.title, "Two mods contain the same code");
                assert_eq!(d.mods, ["luna_minecraft", "e4all"]);
                assert!(d.detail.contains("luna_minecraft and e4all"));
                assert!(d.detail.contains("package link.e4mc"));
                assert!(d.detail.contains("can't be loaded together"));
                assert!(d.detail.contains("Remove or update one"));
                assert!(!d.detail.contains("betterend"));
                assert!(!d.repair);
                assert_eq!(d.fix, None);
            }
        }
    }

    #[test]
    fn resolver_missing_dependencies_name_the_mod_and_version_range() {
        // Either reason line alone must work, as well as the complete resolver block.
        for log in [
            CONNECTOR_RESOLUTION,
            "[HARD_DEP_NO_CANDIDATE betterend 4.0.11 {depends bclib @ [3.0.x]}]",
            "[HARD_DEP betterend 4.0.11 {depends bclib @ [3.0.x]}]",
        ] {
            let d = diagnose(log).unwrap();
            assert_eq!(d.title, "A mod dependency is missing");
            assert_eq!(d.mods, ["betterend", "bclib"]);
            assert_eq!(
                d.detail,
                "betterend needs bclib (version [3.0.x]), which isn't installed."
            );
        }
    }

    #[test]
    fn resolver_hints_ignore_unbounded_placeholder_versions() {
        assert_eq!(update_hint(CONNECTOR_RESOLUTION), None);
        assert!(resolver_updates(CONNECTOR_RESOLUTION).is_empty());
        let sodium = "Fix: add [], remove [], replace [[sodium 0.6.13+mc1.21.1] -> add:sodium 0.8.12 ([[0.8.12,∞)])]";
        assert_eq!(
            update_hint(sodium).as_deref(),
            Some("update sodium to 0.8.12")
        );
        assert_eq!(
            resolver_updates(sodium),
            [("sodium".into(), "0.8.12".into())]
        );
        assert_eq!(
            update_hint(&format!("{CONNECTOR_RESOLUTION}\n{sodium}")).as_deref(),
            Some("update sodium to 0.8.12")
        );
        // A real single-component version with a bounded range is still usable.
        assert_eq!(
            update_hint("[[example 0] -> add:example 1 ([[1,1]])]").as_deref(),
            Some("update example to 1")
        );
        let placeholder = CONNECTOR_RESOLUTION.lines().last().unwrap();
        let d = diagnose(&format!(
            "{placeholder}\nMod 'More Culling' (moreculling) 1.0.10 is incompatible with any version of mod 'Sodium' (sodium)"
        ))
        .unwrap();
        assert_eq!(d.title, "More Culling and Sodium don't work together");
        assert!(!d.detail.contains("update betterend to 1"));
        for (dependency, expected_title) in [
            ("minecraft", "1 mod(s) are for another Minecraft version"),
            ("fabricloader", "Fabric Loader is too old for these mods"),
        ] {
            let d = diagnose(&format!(
                "{CONNECTOR_RESOLUTION}\nHARD_DEP_NO_CANDIDATE example 1.0 {{depends {dependency} @ [>=1.0]}}"
            ))
            .unwrap();
            assert_eq!(d.title, expected_title);
            assert!(!d.detail.contains("update betterend to 1"));
        }
    }

    #[test]
    fn a_real_resolver_update_for_an_installed_dependency_is_preserved() {
        let log = "Reason: [HARD_DEP betterblockentities 1.2.1 {depends sodium @ [>=0.8.12]}]
Fix: add [], remove [], replace [[sodium 0.6.13+mc1.21.1] -> add:sodium 0.8.12 ([[0.8.12,∞)])]";
        let d = diagnose(log).unwrap();
        assert_eq!(d.title, "Some mods need updating together");
        assert!(d.detail.contains("update sodium to 0.8.12"));
        assert!(!d.detail.contains("isn't installed"));
    }

    #[test]
    fn wrong_minecraft_version() {
        let log = "- Mod 'Old Mod' (old) 1.0 requires version 1.20.1 of minecraft, but only the wrong version is present: 1.21.6!";
        let d = diagnose(log).unwrap();
        assert_eq!(d.title, "Old Mod is for a different Minecraft version");
        assert!(d.detail.contains("1.20.1") && d.detail.contains("1.21.6"));
    }

    #[test]
    fn a_mod_that_needs_a_newer_other_mod_says_to_update_that_mod() {
        let log = "[18:37:08] [main/INFO] Fix: add [], remove [], replace [[sodium 0.6.13+mc1.21.1] -> add:sodium 0.8.12 ([[0.8.12,\u{221e})])]
    \t - Mod 'Better Block Entities' (betterblockentities) 1.2.1 requires version 0.8.12 or later of mod 'Sodium' (sodium), but only the wrong version is present: 0.6.13+mc1.21.1!";
        let d = diagnose(log).unwrap();
        assert_eq!(d.title, "Sodium is too old for Better Block Entities");
        assert!(d.detail.contains("update sodium to 0.8.12"), "{}", d.detail);
        assert!(d.detail.contains("0.6.13"), "{}", d.detail);
    }

    #[test]
    fn a_clash_that_an_update_fixes_says_which_update() {
        let log = "[18:37:08] [main/INFO] Fix: add [], remove [], replace [[sodium 0.6.13+mc1.21.1] -> add:sodium 0.8.12 ([[0.8.12,\u{221e})])]
    \t - Mod 'More Culling' (moreculling) 1.0.10 is incompatible with version 0.6.13 or earlier of mod 'Sodium' (sodium), yet a conflicting version is present: 0.6.13+mc1.21.1!";
        let d = diagnose(log).unwrap();
        assert!(d.detail.contains("update sodium to 0.8.12"), "{}", d.detail);
    }

    #[test]
    fn a_game_that_wrote_nothing_is_called_out() {
        let banner = "──── Launching Fabric 1.21 (1.21) as Steve ────\n";
        assert!(
            silent_exit(banner, 1)
                .unwrap()
                .detail
                .contains("exit code 1")
        );
        assert!(silent_exit("[12:00:00] [main/INFO] Loading Minecraft", 1).is_none());
    }

    #[test]
    fn a_translated_fabric_message_is_read_from_the_english_resolver_lines() {
        let log = "[15:09:04] [main/INFO] Loading Minecraft 26.2 with Fabric Loader 0.19.5
[15:09:05] [main/INFO] Immediate reason: [HARD_DEP_NO_CANDIDATE nuit_interop 1.0.0-beta.5+mc26.2 {depends nuit @ [1.0.0-beta.5+mc26.2]}]
[15:09:05] [main/INFO] Reason: [HARD_DEP nuit_interop 1.0.0-beta.5+mc26.2 {depends nuit @ [1.0.0-beta.5+mc26.2]}, HARD_DEP macos_input_fixes 1.16+1.21.11 {depends minecraft @ [<26.1]}]
[15:09:05] [main/INFO] Fix: add [], remove [], replace [[nuit 1.0.0-beta.4+mc26.2] -> add:nuit 1.0.0-beta.5+mc26.2 ([[1.0.0-beta.5+mc26.2,1.0.0-beta.5+mc26.2]])]
[15:09:05] [main/ERROR] Incompatible mods found!";
        let d = diagnose(log).unwrap();
        assert_eq!(d.title, "1 mod(s) are for another Minecraft version");
        assert_eq!(d.mods, ["macos_input_fixes"]);
        assert!(d.detail.contains("26.2"));
        assert!(d.detail.contains("update nuit to 1.0.0-beta.5+mc26.2"));
        assert_eq!(
            d.fix,
            Some(Fix::DisableMods(vec!["macos_input_fixes".into()]))
        );
    }

    #[test]
    fn leftover_mods_for_another_minecraft_version_are_named() {
        let log = "Incompatible mods found!
            	 - Replace mod 'LambDynamicLights (Runtime)' (lambdynlights_runtime) 4.9.1+1.21.11 with version 4.9.1+1.21.11 or later that is compatible with:
            	 - Mod 'SpruceUI' (spruceui) 9.1.0+1.21.11 requires any version between 1.21.11- (inclusive) and 1.22- (exclusive) of 'Minecraft' (minecraft), but only the wrong version is present: 26.3!
            	 - Mod 'Pride Lib' (pride) 1.5.1+1.21.11 requires any version between 1.21.11- (inclusive) and 1.22- (exclusive) of 'Minecraft' (minecraft), but only the wrong version is present: 26.3!";
        let d = diagnose(log).unwrap();
        assert_eq!(
            d.title,
            "2 mods are for a different Minecraft version: SpruceUI, Pride Lib"
        );
        assert_eq!(d.mods, ["SpruceUI", "Pride Lib"]);
        assert!(d.detail.contains("26.3") && d.detail.contains("Switch them off"));
        assert_eq!(
            d.fix,
            Some(Fix::DisableMods(vec![
                "SpruceUI".into(),
                "Pride Lib".into()
            ]))
        );
    }

    #[test]
    fn report_377_names_stale_vanilla_performance_mods_before_missing_libraries() {
        let log = "──── Launching Vanilla (1.8.9) as Player ────
[main/INFO] Loading Minecraft 1.8.9 with Fabric Loader 0.20.1
[main/ERROR] Incompatible mods found!
 - Mod 'EntityCulling' (entityculling) 1.8.2 requires any version of fabric-api, which is missing!
 - Mod 'Fzzy Config' (fzzy_config) 0.7.7+1.21 requires any version of fabric-language-kotlin, which is missing!
 - Mod 'Particle Core' (particle_core) 0.3.3+1.21 requires version 1.21 or later of 'Minecraft' (minecraft), but only the wrong version is present: 1.8.9!
 - Mod 'TRender' (trender) 1.0.5 requires any 1.21.x version of 'Minecraft' (minecraft), but only the wrong version is present: 1.8.9!
 - Mod 'Sodium' (sodium) 0.6.13+mc1.21.1 requires version 1.21.1 of 'Minecraft' (minecraft), but only the wrong version is present: 1.8.9!";
        let (d, matched) = diagnose_match(&format!("{log}\n{log}"), None).unwrap();
        assert_eq!(
            d.title,
            "3 mods are for a different Minecraft version: Particle Core, TRender, Sodium"
        );
        assert_eq!(d.mods, ["Particle Core", "TRender", "Sodium"]);
        assert!(d.detail.contains("Arctic's Performance mods"));
        assert!(d.detail.contains("Vanilla instance"));
        assert!(matched.contains("(minecraft)"));
        assert!(!d.detail.contains("fabric-api"));
        let report = Report::build(
            "1.8.9",
            "",
            "A mod needs fabric-api",
            &[log],
            &[],
            None,
            None,
        );
        assert_eq!(report.title, d.title);
    }

    #[test]
    fn an_old_fabric_loader_is_named() {
        let log = "	 - Mod 'Fabric API' (fabric-api) 0.92.12+1.20.1 requires version 0.16.10 or later of mod 'Fabric Loader' (fabricloader), but only the wrong version is present: 0.15.11!
            	 - Mod 'ModernFix' (modernfix) 5.25.2+mc1.20.1 requires version 0.16.10 or later of mod 'Fabric Loader' (fabricloader), but only the wrong version is present: 0.15.11!";
        let d = diagnose(log).unwrap();
        assert_eq!(d.title, "Fabric Loader is too old for these mods");
        assert_eq!(d.mods, ["Fabric API", "ModernFix"]);
        assert!(d.detail.contains("0.16.10") && d.detail.contains("0.15.11"));
        assert_eq!(d.fix, Some(Fix::UpdateLoader));
    }

    #[test]
    fn java_version() {
        let log = "java.lang.UnsupportedClassVersionError: x has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
        assert_eq!(title(log), "This needs Java 21");
    }

    #[test]
    fn crash_505_explains_the_custom_java_mismatch() {
        let log = include_str!("../tests/fixtures/crash-505-launchwrapper-java.log");
        let generic = "Game exited with code 1 (0x00000001)";
        let expected = "This version needs Java 8, but is running on Java 9 or newer";
        let mods = tempfile::tempdir().unwrap();
        let diagnosis = crate::crash_conflicts::diagnose_exit_in(log, 1, mods.path()).unwrap();
        assert_eq!(diagnosis.title, expected);
        assert_eq!(diagnosis.detail, CUSTOM_JAVA_FIX);
        assert!(!diagnosis.repair);

        let lines = log.lines().collect::<Vec<_>>();
        let report = Report::build("1.8.9", "Forge", generic, &lines, &[], None, None);
        assert_eq!(report.title, expected);
        assert!(report.log.starts_with(
            "\n---- first error ----\njava.lang.ClassCastException: class jdk.internal.loader.ClassLoaders$AppClassLoader cannot be cast to class java.net.URLClassLoader"
        ));
        assert!(
            report
                .log
                .contains("matched: java.lang.ClassCastException:")
        );

        let known = log.replace("status=exit code: 0", "Java 25; status=exit code: 0");
        assert_eq!(
            title(&known),
            "This version needs Java 8, but is running on Java 25"
        );
        let early_java = log
            .replace("Exception: class ", "Exception: ")
            .replace("cast to class ", "cast to ");
        assert_eq!(title(&early_java), expected);
        let module_qualified = early_java
            .replace("Exception: jdk.", "Exception: java.base/jdk.")
            .replace("cast to java.net.", "cast to java.base/java.net.");
        assert_eq!(title(&module_qualified), expected);
        let unrelated = log.replace("java.net.URLClassLoader", "java.lang.String");
        assert!(diagnose(&unrelated).is_none());
        let unrelated = log.replace("net.minecraft.launchwrapper.Launch", "com.example.Main");
        assert!(diagnose(&unrelated).is_none());
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

    const FORGE_SUSPECTS_295: &str = "Suspected Mods:
    LesRaisins Tactical Equipments (lrtactical), Version: 0.4.4
        at TRANSFORMER/lrtactical@0.4.4/example.Grenade.explode(Grenade.java:42)
    Superb Warfare (superbwarfare), Version: 0.8.8
        at TRANSFORMER/superbwarfare@0.8.8/example.LivingEventHandler.onExplosionDetonate(LivingEventHandler.java:42)
Stacktrace:
    at example.Other.frame(Other.java:1)";

    /// Real Fabric 1.20.1 crash #310, trimmed without changing the failure lines. At b7fe0d52
    /// the config rule already names particle_core (the #64 fix), but misses the Sodium clash.
    const REPORT_310: &str = include_str!("../tests/fixtures/crash-310-particle-core.log");

    /// Fabric crash #315 identifies the constant injector displaced by Cursed Craft,
    /// while Lithium also targets the class and AppleSkin appears in the inventory.
    const REPORT_315: &str = include_str!("../tests/fixtures/crash-315-carpet.log");

    #[test]
    fn report_315_names_carpet_without_blaming_appleskin_or_lithium() {
        for text in [REPORT_315.to_owned(), REPORT_315.replace('\n', "\r\n")] {
            let d = diagnose(&text).unwrap();
            assert_eq!(d.title, "Carpet clashes with Cursed Craft");
            assert_eq!(d.mods, ["carpet", "cursed_craft"]);
            assert!(d.detail.contains("Update or remove Carpet"));
            assert!(!d.detail.to_lowercase().contains("appleskin"));
            assert!(!d.detail.to_lowercase().contains("lithium"));
            assert!(!d.repair);
            assert!(d.fix.is_none());
        }
    }

    #[test]
    fn constant_clash_requires_the_failed_injector_and_both_context_entries() {
        for text in [
            REPORT_315.replace("@ModifyConstant conflict.", "Unrelated warning."),
            REPORT_315.replace("::pushLimit(I)I with priority", "::other(I)I with priority"),
            REPORT_315.replace(
                "already redirected by cursed_craft.mixins.json:InfPiston",
                "already redirected by lithium.mixins.json:OtherMixin",
            ),
            REPORT_315.replace(
                "ca.maximilian.cursed_craft.mixin.InfPiston",
                "ca.maximilian.cursed_craft.mixin.OtherMixin",
            ),
            REPORT_315.replace("failure end", ""),
        ] {
            let d = diagnose(&text).unwrap();
            assert_eq!(d.mods, ["carpet"]);
            assert_eq!(d.title, "carpet failed to change the game");
        }
    }

    #[test]
    fn fabric_injection_owner_is_independent_of_injector_wording() {
        for injector in [
            "Constant modifier method",
            "Redirector",
            "Callback method",
            "Variable modifier method",
            "Argument modifier method",
            "Multi-argument modifier method",
            "@ModifyVariable",
            "@ModifyArg",
            "Unknown injector",
        ] {
            for exception in ["InjectionError:", "InvalidInjectionException:"] {
                let text = REPORT_315
                    .replace("Constant modifier method", injector)
                    .replace("InjectionError:", exception);
                let d = diagnose(&text).unwrap();
                assert_eq!(d.mods, ["carpet", "cursed_craft"], "{exception} {injector}");
                assert_eq!(d.title, "Carpet clashes with Cursed Craft");
            }
        }
    }

    #[test]
    fn report_310_names_particle_core_and_sodium_without_blaming_necronomicon() {
        for text in [REPORT_310.to_owned(), REPORT_310.replace('\n', "\r\n")] {
            let d = diagnose(&text).unwrap();
            assert_eq!(d.title, "Particle Core clashes with Sodium");
            assert_eq!(d.mods, ["particle_core", "sodium"]);
            assert!(d.detail.contains("Update or remove Particle Core"));
            assert!(d.detail.contains("compatibility with Sodium"));
            assert!(!d.detail.to_lowercase().contains("necronomicon"));
            assert!(!d.repair);
            assert!(d.fix.is_none());
        }
    }

    const REPORT_323: &str = include_str!("../tests/fixtures/crash-323-title-redirect.log");

    #[test]
    fn report_323_names_the_arctic_redirect_clash() {
        for text in [REPORT_323.to_owned(), REPORT_323.replace('\n', "\r\n")] {
            let (d, matched) = diagnose_match(&text, None).unwrap();
            assert_eq!(d.title, "Arctic Client conflict with mod_id");
            assert_eq!(d.mods, ["mod_id", "arctic"]);
            assert!(d.detail.contains("renderSpace"));
            assert!(d.detail.contains("arctic$backdrop"));
            assert!(!matched.contains("could not be read"));
            assert!(matched.contains("InjectionError"));
            assert!(d.fix.is_none());
            assert!(!d.repair);
        }
    }

    #[test]
    fn redirect_clash_requires_the_same_failing_redirect_and_an_earlier_warning() {
        let (before, failure) = REPORT_323
            .split_once("Caused by: org.spongepowered")
            .unwrap();
        for before in [
            before.replace("swgc.mixins.json", "other.mixins.json"),
            before.replace("MixinTitleScreen", "OtherMixin"),
            before.replace("from mod mod_id", "from mod other"),
            before.replace("renderSpace", "otherHandler"),
            before.replace("class_766;FF", "class_766;F"),
            before.replace("@Redirect conflict", "@ModifyConstant conflict"),
            before.replace("from mod arctic", "from mod mod_id"),
            String::new(),
        ] {
            let text = format!("{before}Caused by: org.spongepowered{failure}");
            let d = diagnose(&text).unwrap();
            assert_eq!(d.title, "mod_id failed to change the game");
            assert_eq!(d.mods, ["mod_id"]);
        }
        let late = format!("Caused by: org.spongepowered{failure}\n{before}");
        assert_eq!(title(&late), "mod_id failed to change the game");
        assert!(diagnose(before).is_none());
    }

    #[test]
    fn redirect_clashes_name_both_mods_when_neither_is_arctic() {
        let d = diagnose(&REPORT_323.replace("arctic", "scenery")).unwrap();
        assert_eq!(d.title, "mod_id clashes with scenery");
        assert_eq!(d.mods, ["mod_id", "scenery"]);
    }

    #[test]
    fn unreadable_refmap_is_never_the_matched_mixin_evidence() {
        // Without an explicit failure owner, the generic config rule matches first.
        let text = REPORT_323.replace(" from mod mod_id failed", " failed");
        let (d, matched) = diagnose_match(&text, None).unwrap();
        assert_eq!(d.title, "Arctic Client conflict with mod_id");
        assert!(matched.contains("InjectionError"));
        assert!(!matched.contains("could not be read"));
        let warning = REPORT_323.lines().next().unwrap();
        assert!(diagnose(warning).is_none());
        let text =
            format!("{warning}\nInvalidMixinException: invalid broken.mixins.json:BrokenMixin");
        let (d, matched) = diagnose_match(&text, None).unwrap();
        assert_eq!(d.mods, ["broken"]);
        assert!(matched.starts_with("InvalidMixinException:"));
    }

    #[test]
    fn fabric_root_injection_owner_wins_over_outer_mixin_blame() {
        for outer in [
            "Mixin apply for mod necronomicon failed necronomicon.mixins.json:OuterMixin",
            "Mixin [necronomicon.mixins.json:OuterMixin] from mod necronomicon failed",
            "Caused by: InvalidInjectionException: invalid necronomicon.mixins.json:OuterMixin from mod necronomicon",
        ] {
            for exception in ["InjectionError:", "InvalidInjectionException:"] {
                let text = format!("{outer}\n{REPORT_310}").replace("InjectionError:", exception);
                let d = diagnose(&text).unwrap();
                assert_eq!(d.mods, ["particle_core", "sodium"], "{outer}");
                assert_eq!(d.title, "Particle Core clashes with Sodium");
            }
        }
    }

    #[test]
    fn fabric_mixin_clash_requires_missing_targets_and_matching_context() {
        for text in [
            REPORT_310.replace("Scanned 0 target(s)", "Scanned 1 target(s)"),
            REPORT_310.replace("class=net.minecraft.class_3940;", "class=net.minecraft.Other;"),
            REPORT_310.replace("me.fzzyhmstrs.particle_core.mixins.BillboardParticleVertexMixin", "me.fzzyhmstrs.particle_core.mixins.OtherMixin"),
            REPORT_310.replace("sodium.mixins.json:me.jellysquid.mods.sodium.mixin.features.render.particle.BillboardParticleMixin", "particle_core.mixins.json:OtherMixin"),
            REPORT_310.replace("failure end", ""),
            REPORT_310.replace("selected target mixin count=2", "selected target mixin count=3\nthird.mixins.json:ThirdMixin"),
            REPORT_310.split_once("---- crash report ----").unwrap().1.to_owned(),
        ] {
            let d = diagnose(&text).unwrap();
            assert_eq!(d.mods, ["particle_core"]);
            assert_eq!(d.title, "particle_core failed to change the game");
        }
        let missing = REPORT_310.replace(
            "Scanned 0 target(s)",
            "could not find any targets matching 'method_3074'",
        );
        assert_eq!(
            diagnose(&missing).unwrap().mods,
            ["particle_core", "sodium"]
        );
    }

    #[test]
    fn fabric_mixin_context_is_scoped_to_one_failure_in_both_log_formats() {
        let (before, context) = REPORT_310.split_once("failure begin").unwrap();
        let (context, after) = context.split_once("failure end").unwrap();
        let prefixed = context
            .lines()
            .map(|l| format!("[Arctic mixin] {l}\n"))
            .collect::<String>();
        let text = format!(
            "{before}[Arctic mixin] failure begin\n{prefixed}[Arctic mixin] failure end{after}"
        );
        assert_eq!(diagnose(&text).unwrap().mods, ["particle_core", "sodium"]);

        // Do not borrow Sodium from a separate failure on the same class.
        let split = context.replace("sodium.mixins.json:", "failure end\nfailure begin\nclass=net.minecraft.class_3940; thread=Render thread\ntargeting mixins (config:class; includes failing mixin):\nsodium.mixins.json:");
        let text = format!("{before}failure begin\n{split}failure end{after}");
        assert_eq!(diagnose(&text).unwrap().mods, ["particle_core"]);

        let text = format!("failure begin\nincomplete context\nfailure end\n{REPORT_310}");
        assert_eq!(diagnose(&text).unwrap().mods, ["particle_core", "sodium"]);
    }

    #[test]
    fn forge_304_array_owner_is_a_mixin_failure() {
        let text = "java.lang.NoClassDefFoundError: net/minecraft/client/renderer/texture/TextureAtlasSprite
    at TextureMap.<init>(TextureMap.java:64)
Caused by: org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError: An unexpected critical error was encountered
Caused by: java.lang.RuntimeException: java.lang.ClassNotFoundException: [Ljava.awt.image.BufferedImage;
    at org.spongepowered.asm.mixin.transformer.MixinPreProcessorStandard.transformMethod(MixinPreProcessorStandard.java:682)
    at org.spongepowered.asm.mixin.transformer.MixinPreProcessorStandard.transform(MixinPreProcessorStandard.java:670)
    at org.spongepowered.asm.mixin.transformer.MixinPreProcessorStandard.attach(MixinPreProcessorStandard.java:267)
    at org.spongepowered.asm.mixin.transformer.MixinPreProcessorStandard.createContextFor(MixinPreProcessorStandard.java:241)
    at org.spongepowered.asm.mixin.transformer.MixinInfo.createContextFor(MixinInfo.java:1144)
    at org.spongepowered.asm.mixin.transformer.MixinApplicatorStandard.apply(MixinApplicatorStandard.java:267)";
        let diagnosis = diagnose(text).unwrap();
        assert_eq!(diagnosis.title, "A mod's mixin failed to apply");
        assert!(diagnosis.mods.is_empty());
        let named = format!(
            "{text}\nMixinTransformerError: arctic-legacy.mixins.json:GlassLegacyImageMixin from mod arctic"
        );
        let diagnosis = diagnose(&named).unwrap();
        assert!(diagnosis.title.contains("arctic"));
        assert!(diagnosis.detail.contains("mixin failed to apply"));
    }

    #[test]
    fn forge_295_mixin_owner_wins_over_triggering_mods() {
        let failure = "org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError
Caused by: org.spongepowered.asm.mixin.injection.throwables.InjectionError: Critical injection failure: Redirector superbaddon$modifyExplosionKnockbackForce(Lnet/minecraft/world/entity/LivingEntity;D)D in superbaddon.mixins.json:CustomExplosionMixin failed injection check, (0/1) succeeded. Scanned 1 target(s). Using refmap superbaddon.refmap.json";
        let d = diagnose(&format!("{FORGE_SUSPECTS_295}\n{failure}")).unwrap();
        assert_eq!(d.mods, ["superbaddon"]);
        assert_eq!(d.title, "superbaddon failed to change the game");
        assert!(d.detail.contains("mixin doesn't fit"));
        assert!(
            d.detail.contains(
                "Update superbaddon (or the mod it's an addon for), or remove superbaddon"
            )
        );
    }

    #[test]
    fn generic_mixin_exception_shapes_identify_the_config_owner() {
        for failure in [
            "InvalidInjectionException: target not found superbaddon.mixins.json:CustomExplosionMixin",
            "InvalidMixinException: invalid superbaddon.mixins.json:CustomExplosionMixin",
        ] {
            assert_eq!(diagnose(failure).unwrap().mods, ["superbaddon"]);
        }
        assert!(diagnose("Loaded superbaddon.mixins.json successfully").is_none());
        assert!(
            diagnose("Mixin [ordinary log message\ncontinued] from mod necronomicon").is_none()
        );
    }

    #[test]
    fn mixin_warnings_need_evidence_of_a_failure() {
        for message in [
            "@Mixin target net.minecraft.Foo was not found optional.mixins.json:FooMixin",
            "Mixin apply for mod optional failed optional.mixins.json:FooMixin",
            "Mixin [optional.mixins.json:FooMixin] from mod optional",
            "MixinTransformerError: optional.mixins.json:FooMixin caused by ClassNotFoundException",
            "InjectionError: Critical injection failure in optional.mixins.json:FooMixin from mod optional",
        ] {
            for prefix in ["[16:53:41] [main/WARN]:", "[main/WARN]", "[WARN]"] {
                let log = format!("{prefix} {message}");
                assert!(diagnose(&log).is_none(), "{log}");
                assert!(crate::crash_conflicts::mixin_failure_line(&log).is_none());
            }
        }
        let log = "[main/WARN] @Mixin target net.minecraft.Foo was not found optional.mixins.json:FooMixin\n\
                   Caused by: org.spongepowered.asm.mixin.transformer.throwables.InvalidMixinException: @Mixin target net.minecraft.Foo was not found required.mixins.json:FooMixin";
        assert_eq!(diagnose(log).unwrap().mods, ["required"]);
    }

    #[test]
    fn optional_mixin_warnings_never_supply_mixin_failure_evidence() {
        for warning in [
            HANG_516
                .lines()
                .find(|line| line.contains("@Mixin target "))
                .unwrap(),
            "@Mixin target net.minecraft.Foo was not found optional.mixins.json:FooMixin",
            "@Mixin target example.InvalidMixinException was not found optional.mixins.json:FooMixin",
            "[mixin] @Mixin target example.InvalidMixinException was not found optional.mixins.json:FooMixin from mod optional",
            "[main/WARNING] InvalidMixinException: target not found optional.mixins.json:FooMixin",
            "[main/WARN] InvalidMixinException: target not found optional.mixins.json:FooMixin",
            "[main/WARN] Mixin [optional.mixins.json:FooMixin] from mod optional failed",
            "[main/WARN] Mixin apply for mod optional failed optional.mixins.json:FooMixin",
            "[main/WARN] Mixin [optional.mixins.json:FooMixin] from mod optional skipped",
        ] {
            assert!(diagnose(warning).is_none(), "{warning}");
            let mods = tempfile::tempdir().unwrap();
            assert!(
                crate::crash_conflicts::diagnose_exit_in(warning, 1, mods.path()).is_none(),
                "{warning}"
            );
            let failure = "InvalidMixinException: target not found broken.mixins.json:BrokenMixin";
            let (_, matched) = diagnose_match(&format!("{warning}\n{failure}"), None).unwrap();
            assert_eq!(matched, failure);
        }
    }

    #[test]
    fn forge_295_suspected_mods_include_every_entry_and_keep_versions() {
        for report in [
            FORGE_SUSPECTS_295.to_owned(),
            FORGE_SUSPECTS_295.replace('\n', "\r\n"),
        ] {
            let d = diagnose(&report).unwrap();
            assert_eq!(
                d.mods,
                [
                    "LesRaisins Tactical Equipments (lrtactical), Version: 0.4.4",
                    "Superb Warfare (superbwarfare), Version: 0.8.8",
                ]
            );
        }
        let d =
            diagnose("Suspected Mod: First (first)\n    Second (second)\n\n    Unrelated (other)")
                .unwrap();
        assert_eq!(d.mods, ["First (first)", "Second (second)"]);
        assert!(diagnose("Suspected Mods: None\n\n    Unrelated (other)").is_none());
    }

    #[test]
    fn a_mod_built_for_another_minecraft_version_says_so() {
        let log =
            "Mixin apply for mod reeses-sodium-options failed reeses-sodium-options.mixins.json
            	minecraft: Minecraft 1.21
            	reeses-sodium-options: Reese's Sodium Options 1.8.3+mc1.21.4
            	sodium: Sodium 0.8.13+mc1.21.1
";
        let d = diagnose(log).unwrap();
        assert_eq!(
            d.title,
            "reeses-sodium-options is for another Minecraft version"
        );
        assert!(
            d.detail.contains("built for Minecraft 1.21.4"),
            "{}",
            d.detail
        );
        assert!(d.detail.contains("this game is 1.21"), "{}", d.detail);
        // 1.21.1 builds run on 1.21: not a mismatch.
        let near = "Mixin apply for mod sodium failed sodium.mixins.json
            	minecraft: Minecraft 1.21
            	sodium: Sodium 0.8.13+mc1.21.1
";
        assert_eq!(diagnose(near).unwrap().title, "sodium failed to load");
    }

    #[test]
    fn huge_logs_are_cut_on_a_character_boundary() {
        let mut log = "é".repeat(MAX_TEXT);
        log.push_str("java.lang.OutOfMemoryError");
        assert_eq!(title(&log), "Minecraft ran out of memory");
    }

    const SHARED_LOG_LIMIT: usize = 300 * 1024;
    const ANONYMOUS_PC: Own = Own {
        home: None,
        user: None,
    };

    #[test]
    fn shared_log_collapses_tick_spam_before_applying_the_cap() {
        let mods: String = (0..187)
            .map(|i| format!("  mod_{i}: A long mod display name with its version and entrypoint information\n"))
            .collect();
        let startup = format!("[16:35:00] Java 21, GL renderer loaded, 187 mods\n{mods}");
        let error = "[16:35:01] [Render thread/ERROR]: Shader failed\n";
        let message = " [Server thread/INFO] [STDOUT/]: [net.thebrokenscript.plus.procedures.BrokenSunEventProcedure:tryAdvanceDay:33]: [TBS PLUS] Initialized day tracker: 0\n";
        let log = format!(
            "{startup}{error}[16:35:02]{message}{}",
            format!("[16:37:25]{message}").repeat(1907)
        );
        assert!(log.len() > SHARED_LOG_LIMIT);
        assert_eq!(
            shared_log(&log, SHARED_LOG_LIMIT, &ANONYMOUS_PC),
            format!(
                "{startup}{error}[16:35:02]{message}... repeated 1907 more times (until 16:37:25)\n"
            )
        );
    }

    #[test]
    fn shared_log_collapses_short_blocks_and_preserves_a_partial_repeat() {
        for width in 2..=4 {
            let block: String = (0..width).map(|i| format!("message {i}\r\n")).collect();
            let first = block.replace("message", "[16:35:02] message");
            let later = block.replace("message", "[16:35:03] message");
            let partial = "[16:35:04] message 0";
            let log = format!("{first}{later}{later}{partial}");
            assert_eq!(
                shared_log(&log, SHARED_LOG_LIMIT, &ANONYMOUS_PC),
                format!(
                    "{first}... preceding {width} lines repeated 2 more times (until 16:35:03)\r\n{partial}"
                )
            );
        }
        assert_eq!(
            shared_log("same\nsame", SHARED_LOG_LIMIT, &ANONYMOUS_PC),
            "same\n... repeated 1 more times"
        );
    }

    #[test]
    fn shared_log_keeps_complete_head_and_tail_lines_within_the_cap() {
        let log: String = (0..8_000)
            .map(|i| format!("{i:04} {}\n", "line content ".repeat(5)))
            .collect();
        let sent = shared_log(&log, SHARED_LOG_LIMIT, &ANONYMOUS_PC);
        let (head, rest) = sent.split_once("--- ").unwrap();
        let (size, tail) = rest.split_once(" KB cut from the middle ---\n").unwrap();
        assert!(sent.len() <= SHARED_LOG_LIMIT);
        assert!((59 * 1024..=60 * 1024).contains(&head.len()));
        assert!(head.ends_with('\n') && log.starts_with(head));
        assert!(log.ends_with(tail));
        assert_eq!(log.as_bytes()[log.len() - tail.len() - 1], b'\n');
        assert_eq!(
            size.parse::<usize>().unwrap(),
            (log.len() - head.len() - tail.len()).div_ceil(1024)
        );
    }

    #[test]
    fn shared_log_preserves_small_logs_without_repeats() {
        let log = "[16:35:00] Java 21\r\n[16:35:01] GL renderer loaded\r\n[bad time] An error";
        assert_eq!(shared_log(log, log.len(), &ANONYMOUS_PC), log);
        assert_eq!(scrubbed_shared_log("", SHARED_LOG_LIMIT), "");
    }

    #[test]
    fn shared_log_scrubs_both_ends_before_cutting_and_keeps_distinct_lines() {
        let own = Own {
            home: Some(r"C:\Users\PlayerName".into()),
            user: Some("PlayerName".into()),
        };
        let startup = "Started at C:\\Users\\PlayerName\\games\n";
        let middle: String = (0..100).map(|i| format!("Loading mod {i}\n")).collect();
        let tail = "--accessToken\nfirst_private_access_token\n--accessToken\nsecond_private_access_token\nPlayerName left";
        let sent = shared_log(&format!("{startup}{middle}{tail}"), 240, &own);
        assert!(sent.starts_with("Started at <home>\\games\n"));
        assert!(sent.contains("cut from the middle"));
        assert!(sent.ends_with("--accessToken\n<hidden>\n--accessToken\n<hidden>\n<user> left"));
    }

    #[test]
    fn shared_log_is_utf8_safe_even_for_tiny_caps_and_long_lines() {
        let log = format!("起動\n{}\nエラー", "界".repeat(100));
        for max in 0..=log.len() {
            let sent = shared_log(&log, max, &ANONYMOUS_PC);
            assert!(sent.len() <= max);
            assert!(!sent.contains('\u{fffd}'));
        }
        let sent = shared_log(&log, 60, &ANONYMOUS_PC);
        assert!(sent.starts_with("起動\n"));
        assert!(sent.ends_with("エラー"));
        let single_line = "界".repeat(100);
        let sent = shared_log(&single_line, 42, &ANONYMOUS_PC);
        assert!(sent.ends_with('界'));
        assert!(sent.len() <= 42);
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
    fn large_pack_inventory_survives_report_and_transport_limits() {
        let mut written = format!("{}\n\tFabric Mods:\n", "stack frame\n".repeat(3000));
        for i in 0..500 {
            written.push_str(&format!(
                "\t\tmod_{i}: A long display name for mod {i} 1.2.3.4+build-{i}\n"
            ));
        }
        written.push_str("\tLaunched Version: 26.3\n");
        let noise = "noise".repeat(4000);
        let context = "[Arctic mixin] class=net.minecraft.world.entity.item.ItemEntity; role=preloader\n\
            [Arctic mixin] cache lookup=miss\n[Arctic mixin] tick()V\n\
            [Arctic mixin] architectury.mixins.json:MixinItemEntity\n\
            [Arctic mixin] other.json:OtherItemMixin";
        let report = Report::build(
            "26.3",
            "fabric",
            "Crash",
            &[context, &noise],
            &[&noise],
            Some(&written),
            Some(&noise),
        );
        for i in 0..500 {
            assert!(
                report.log.contains(&format!("mod_{i} 1.2.3.4+build-{i}\n")),
                "missing mod {i}"
            );
        }
        assert!(report.log.contains("tick()V"));
        assert!(report.log.contains("other.json:OtherItemMixin"));
        assert!(report.log.len() < 48 * 1024);
        assert!(!report.log.contains("Launched Version 26.3"));
    }

    #[test]
    fn exceptionally_large_inventory_is_lossless_instead_of_cut_off() {
        use base64::Engine as _;
        use std::io::Read as _;
        let mods: String = (0..3000)
            .map(|i| format!("mod_{i} 1.2.3.4+build-{i}\n"))
            .collect();
        let section = mod_inventory_section(&mods);
        assert!(section.contains("gzip+base64"));
        let encoded = section.lines().nth(1).unwrap();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let mut decoded = String::new();
        flate2::read::GzDecoder::new(bytes.as_slice())
            .read_to_string(&mut decoded)
            .unwrap();
        assert_eq!(mods, decoded);
        assert!(section.len() < 24 * 1024);
    }

    #[test]
    fn startup_and_agent_inventory_cover_early_crashes_and_nested_mods() {
        let output = "[main/INFO] Loading 4 mods:\n\t- arctic 1.0.0\n\
            \t- architectury 22.0.5\n\t   |-- nested 2.0.0\n\t   \\-- last 3.0.0\n\
            [main/INFO] other message\n- unrelated line\n\
            [Arctic mixin mod] architectury 22.0.5+resolved\n\
            [Arctic mixin mod] polarium 0.31.3";
        let mods = compact_mod_inventory(output, "");
        assert!(mods.contains("arctic 1.0.0\n"));
        assert!(mods.contains("architectury 22.0.5+resolved\n"));
        assert!(mods.contains("nested 2.0.0\n"));
        assert!(mods.contains("last 3.0.0\n"));
        assert!(mods.contains("polarium 0.31.3\n"));
        assert!(!mods.contains("unrelated"));
    }

    #[test]
    fn agent_evidence_is_deduplicated_and_still_scrubbed() {
        let context = "[Arctic mixin] class=example.Item; role=main\n\
            [Arctic mixin] tick()V\n[Arctic mixin] error=message to user@example.com";
        let noise = "unrelated\n".repeat(4000);
        let report = Report::build(
            "26.3",
            "fabric",
            "Crash",
            &[context, context, &noise],
            &[],
            None,
            None,
        );
        assert_eq!(
            mixin_failure_evidence(&format!("{context}\n{context}"))
                .matches("tick()V")
                .count(),
            1
        );
        assert!(report.log.contains("class=example.Item; role=main"));
        assert!(report.log.contains("tick()V"));
        assert!(!report.log.contains("user@example.com"));
    }

    #[test]
    fn each_failed_class_keeps_its_own_complete_method_list() {
        let a = "[Arctic mixin] failure begin\n[Arctic mixin] class=A\n[Arctic mixin] tick()V\n[Arctic mixin] failure end";
        let b = a.replace("class=A", "class=B");
        let evidence = mixin_failure_evidence(&format!("{a}\n{b}\n{a}\n{b}"));
        assert_eq!(evidence.matches("tick()V").count(), 2);
        assert_eq!(evidence.matches("failure begin").count(), 2);
        assert_eq!(evidence.matches("failure end").count(), 2);
    }

    #[test]
    fn the_games_session_line_never_leaves_with_its_token() {
        let own = Own {
            home: None,
            user: None,
        };
        // 1.8.9's latest.log: a Microsoft token (a JWT), an old Mojang one, and an offline player.
        let jwt = "eyJraWQiOiJhYzg0YSJ9.eyJ4dWlkIjoiMjUzNTQyIn0.c2lnbmF0dXJlLXBhcnQ";
        let id = "593fd4c3951f39ada7bd19ddba5fcb41";
        for token in [jwt, "0123456789abcdef0123456789abcdef", "0"] {
            let text = format!(
                "[00:16:35] [Client thread/INFO]: Setting user: Steve\n\
                 [00:16:35] [Client thread/INFO]: (Session ID is token:{token}:{id})\n\
                 [00:16:36] [Client thread/INFO]: LWJGL Version: 2.9.4"
            );
            let clean = scrub(&text, &own);
            assert!(
                clean.contains("(Session ID is <hidden>)\n"),
                "not masked: {clean}"
            );
            assert!(!clean.contains(id), "player id left in: {clean}");
            if token.len() > 1 {
                assert!(!clean.contains(token), "token left in: {clean}");
            }
            assert!(clean.contains("LWJGL Version: 2.9.4"));
        }
        // Cut off mid-line (the report keeps a bounded tail): still nothing of the token.
        let cut = scrub(&format!("(Session ID is token:{jwt}"), &own);
        assert_eq!(cut, "(Session ID is <hidden>)");
    }

    #[test]
    fn early_join_exception_survives_the_tail_once_and_report_fits() {
        let npe = "java.lang.NullPointerException: Cannot invoke \"Object.x()\"\n\tat net.minecraft.class_634.method_11109(class_634.java:1)\n\tat net.minecraft.class_634.method_11154(class_634.java:2)";
        let mut log = vec![
            "[12:00:01] [main/INFO]: Starting".to_owned(),
            "[12:00:02] [Render thread/ERROR]: Error on GameJoin".to_owned(),
            "java.lang.NullPointerException: join".to_owned(),
            "\tat com.example.mixin.JoinMixin.onJoin(JoinMixin.java:42)".to_owned(),
            "\tat net.minecraft.class_634.method_11120(class_634.java:9)".to_owned(),
            "Caused by: java.lang.IllegalStateException: boom".to_owned(),
            "\tat com.example.Deep.run(Deep.java:3)".to_owned(),
        ];
        for i in 0..400 {
            log.push(format!(
                "[12:01:{:02}] [Render thread/INFO]: <Bob> {}",
                i % 60,
                "spam ".repeat(40)
            ));
            log.push("[12:01:00] [Render thread/ERROR]: Error executing task on Client".to_owned());
            log.extend(npe.lines().map(str::to_owned));
        }
        let lines: Vec<&str> = log.iter().map(String::as_str).collect();
        let all = lines.join("\n");
        assert!(all.len() > REPORT_LOG_BYTES * 4);
        let report = Report::build("1.16.5", "fabric", "", &lines, &[], None, None);
        let section = first_exceptions(&all);
        assert!(report.log.contains(&section));
        assert!(section.contains("JoinMixin.onJoin(JoinMixin.java:42)"));
        assert!(section.contains("Caused by: java.lang.IllegalStateException"));
        assert_eq!(section.matches("Error executing task on Client").count(), 1);
        assert!(report.log.len() <= 48 * 1024);
        assert_eq!(
            first_exceptions(&all).matches("Error on GameJoin").count(),
            1
        );
    }

    #[test]
    fn oversized_crash_stacks_keep_later_causes_and_worker_frames() {
        let long_stack =
            "\tat knot//net.minecraft.class_638.method_2941(class_638.java:364)\n".repeat(1200);
        let written = include_str!("../tests/fixtures/threading-detector-with-worker.log")
            .replace("Caused by:", &format!("{long_stack}Caused by:"))
            .replace(
                "Particle worker #1:",
                &format!("{long_stack}Particle worker #1:"),
            );
        let report = Report::build("1.20.1", "fabric", "", &[], &[], Some(&written), None);
        assert!(report.log.len() <= REPORT_BYTES);
        assert!(report.log.contains("[crash report section truncated]"));
        assert!(report.log.contains("Example inner cause"));
        assert!(report.log.contains("-- Head --\nThread: Render thread"));
        assert!(report.log.contains("-- Thread dumps --"));
        assert!(report.log.contains("Particle worker #1:"));
        assert!(
            report
                .log
                .contains("BlockSampler.sample(BlockSampler.java:17)")
        );
        assert!(report.log.contains("Mixins in Stacktrace:"));
        assert!(report.log.contains("Suspected Mods:"));
    }

    #[test]
    fn oversized_thread_dump_keeps_inventory_and_first_error_in_the_upload() {
        let stack =
            "\tat knot//net.minecraft.class_638.method_2941(class_638.java:364)\n".repeat(45);
        let threads: String = (0..20).map(|i| format!("Worker #{i}:\n{stack}")).collect();
        let written = include_str!("../tests/fixtures/threading-detector-with-worker.log").replace(
            "Particle worker #1:",
            &format!("{threads}Particle worker #1:"),
        );
        let registry = "[Render thread/WARN] Registry minecraft:item ID mismatch";
        let log = [
            registry,
            "[Arctic mixin] example.mixins.json:ExampleMixin",
            "[Render thread/INFO]: Latest game output",
        ];
        let report = Report::build("1.20.1", "fabric", "", &log, &[], Some(&written), None);

        assert!(report.log.len() <= REPORT_BYTES);
        assert!(report.log.contains(
            "---- first error ----\nCaused by: java.lang.IllegalStateException: Example inner cause"
        ));
        assert!(report.log.contains(
            "---- full mod list (id version) ----\nrender_hooks 1.0.0\nscheduler 3.0.0\nspark_particles 2.0.0\n"
        ));
        assert!(
            report
                .log
                .contains(&format!("---- registry failure context ----\n{registry}"))
        );
        assert!(
            report
                .log
                .contains("---- mixin failure context ----\nexample.mixins.json:ExampleMixin")
        );
        assert!(
            report
                .log
                .contains("[Render thread/INFO]: Latest game output")
        );
        assert!(report.log.contains("-- Thread dumps --"));
        assert!(report.log.contains("[crash report section truncated]"));
        assert_eq!(diagnose(&report.log).unwrap().mods, ["spark_particles"]);
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
            &lines,
            Some(&"y".repeat(200_000)),
            Some(&"z".repeat(50_000)),
        );
        assert!(report.log.len() < 48 * 1024);
        assert!(report.log.contains("---- launcher log ----"));
        assert!(report.log.contains("---- crash report ----"));
        assert!(report.log.contains("---- java ----"));
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;

    #[test]
    fn native_library_ownership_uses_the_path_or_the_first_java_frame() {
        let fixture = include_str!("../tests/fixtures/crash-441-native-mod.log");
        let no_package_match = fixture.replace("io.homo.superresolution.", "example.other.");
        let no_path = fixture.replace("config/super_resolution/", "native/");
        let jar_path = no_package_match.replace(
            "config/super_resolution/",
            r"mods\super_resolution-0.9.2-alpha.2+opengl.jar!\",
        );
        for text in [&no_package_match, &no_path, &jar_path] {
            assert_eq!(diagnose(text).unwrap().mods, ["super_resolution"]);
        }

        let windows = no_path
            .replace("SIGILL (0x4)", "EXCEPTION_ILLEGAL_INSTRUCTION (0xc000001d)")
            .replace(
                "libSuperResolution+linux64+release.so",
                "superresolution.dll",
            );
        let diagnosis = diagnose(&windows).unwrap();
        assert!(diagnosis.title.contains("superresolution.dll"));
        assert!(diagnosis.detail.contains("doesn't support"));

        let segfault = windows.replace(
            "EXCEPTION_ILLEGAL_INSTRUCTION",
            "EXCEPTION_ACCESS_VIOLATION",
        );
        assert!(
            diagnose(&segfault)
                .unwrap()
                .detail
                .contains("EXCEPTION_ACCESS_VIOLATION")
        );

        let jvm = fixture.replace("libSuperResolution+linux64+release.so", "libjvm.so");
        assert_eq!(diagnose(&jvm).unwrap().title, "The Java runtime crashed");
    }

    #[test]
    fn fatal_java_error_uses_the_crashing_thread_or_compile_task() {
        let fatal = format!("{JAVA_FATAL_HEADER}\n#\n#  EXCEPTION_ACCESS_VIOLATION (0xc0000005)");
        for clue in [
            "Current thread (0x1): JavaThread \"C1 CompilerThread0\"",
            "Current thread (0x1): JavaThread \"C2 CompilerThread5\"",
            "Current CompileTask:\nC2: 123 4 example.Class::method (10 bytes)",
        ] {
            let text = format!("{fatal}\n\n{clue}");
            let diagnosis = diagnose(&text).unwrap();
            assert_eq!(diagnosis.title, "Java's JIT compiler crashed");
            assert!(diagnosis.detail.contains("runtime Arctic recommends"));
            assert!(diagnosis.detail.contains("test your RAM"));
            if clue.starts_with("Current CompileTask:") {
                assert!(diagnosis.detail.contains("example.Class::method"));
            }
        }
        let text = format!(
            "[main/WARN]: Mixin apply for mod optional failed optional.mixins.json:FooMixin\n{fatal}\nCurrent thread (0x1): JavaThread \"Render thread\""
        );
        assert_eq!(
            diagnose(&text).unwrap().title,
            "Minecraft crashed in native code"
        );
    }

    #[test]
    fn report_300_native_exit_outranks_resource_pack_and_logged_linkage() {
        let text = include_str!("../tests/fixtures/crash-300-failfast.log");
        assert!(!text.contains("NoSuchMethodError"));
        // Independently verify the stacked linkage fix, without native-exit precedence.
        let without_exit = text
            .lines()
            .filter(|l| !l.contains("exit code"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            diagnose(&without_exit).is_none_or(|d| d.title != "A mod is for a different version")
        );
        let d = diagnose(text).unwrap();
        assert_eq!(d.title, "Minecraft crashed in native code");
        assert!(d.detail.contains("unknown"));
        let noisy =
            format!("[main/ERROR] java.lang.NoSuchMethodError: optional.Integration\n{text}");
        assert_eq!(diagnose(&noisy).unwrap().title, d.title);
        let report = Report::build(
            "1.21.11",
            "Fabric",
            "A mod is for a different version",
            &[text],
            &[],
            None,
            None,
        );
        assert_eq!(report.title, d.title);
        assert!(report.log.contains("matched:"));
    }
    #[test]
    fn native_modules_and_existing_crash_documents() {
        for code in [0xC0000005u32, 0xC0000409, 0xC0000374, 0xC000001D] {
            let text = format!("exit code {} (0x{code:08X})", code as i32);
            assert!(diagnose(&text).unwrap().title.contains("native code"));
            for (module, advice) in [
                ("nvoglv64.dll", "graphics driver"),
                ("atio6axx.dll", "graphics driver"),
                ("ig9icd64.dll", "graphics driver"),
                ("jvm.dll", "Java runtime"),
                ("mod-native.dll", "mod"),
            ] {
                let text = format!(
                    "{text}\n[Arctic diagnostics] native fault: module={module}; version=1; exception=0xc0000409; offset=0x123"
                );
                let d = diagnose(&text).unwrap();
                assert!(d.title.contains(module));
                assert!(d.detail.contains(advice));
                let r = Report::build("1", "Fabric", "unknown", &[&text], &[], None, None);
                assert!(r.log.contains("---- diagnostics ----"));
                assert!(r.log.contains("offset=0x123"));
                assert!(
                    native_diagnosis(&format!(
                        "{text}\nA fatal error has been detected by the Java Runtime Environment"
                    ))
                    .is_none()
                );
                assert!(
                    native_diagnosis(&format!("{text}\n---- Minecraft Crash Report ----"))
                        .is_none()
                );
            }
        }
        let owned = "exit code -1073740791 (0xC0000409)\n[Arctic diagnostics] native fault: module=mod-native.dll; version=1; exception=0xc0000409; offset=0x123; bundled mod=voicechat";
        let d = diagnose(owned).unwrap();
        assert!(d.title.contains("voicechat"));
        assert!(d.detail.contains("bundled with voicechat"));
        assert!(native_diagnosis("exit code 1 (0x00000001)").is_none());
    }
}
