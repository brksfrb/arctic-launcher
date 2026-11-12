//! Plain-language explanations for common Minecraft crashes, read from the
//! game log and crash report: missing or wrong-version mods, broken mixins,
//! the wrong Java, running out of memory, graphics drivers.

use std::sync::OnceLock;

use regex::Regex;

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
            fix: None,
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
        "Minecraft is starting with {}{computer} for {mods} mod files. A large modpack may need more memory. If it runs out, try fewer mods or close other programs.",
        gb(u64::from(given_mb))
    ))
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
        // Fabric's newer wording, with names in quotes and ranges:
        //  "- Mod 'SpruceUI' (spruceui) 9.1 requires any version between 1.21.11- (inclusive) and 1.22- (exclusive) of 'Minecraft' (minecraft), but only the wrong version is present: 26.3!"
        //  "- Mod 'ModernFix' (modernfix) 5.2 requires version 0.16.10 or later of mod 'Fabric Loader' (fabricloader), but only the wrong version is present: 0.15.11!"
        pattern: r"Mod '([^']+)' \(([^)]+)\) \S+ requires (?:any version|version) (.+?) of (?:mod )?'[^']+' \((minecraft|fabricloader)\), but only the wrong version is present: ([^!\s]+)",
        explain: |c, text| {
            // Every mod the same complaint names, once each.
            let target = c[4].to_owned();
            let mut names: Vec<String> = Vec::new();
            if let Ok(re) = Regex::new(&format!(
                r"Mod '([^']+)' \([^)]+\) \S+ requires [^
]*? \({target}\), but only the wrong version is present"
            )) {
                for m in re.captures_iter(text) {
                    let name = m[1].to_owned();
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
            if names.is_empty() {
                names.push(c[1].to_owned());
            }
            let have = &c[5];
            if target == "minecraft" {
                Diagnosis {
                    fix: Some(Fix::DisableMods(names.clone())),
                    ..Diagnosis::new(
                        format!("{} mod(s) are for another Minecraft version", names.len()),
                        format!(
                            "These mods are made for another Minecraft version than {have}: {}. They are probably left over from another version (an instance copied or switched to a newer game). Switch them off, or update them from the instance's Mods page.",
                            names.join(", ")
                        ),
                        names,
                    )
                }
            } else {
                Diagnosis {
                    fix: Some(Fix::UpdateLoader),
                    ..Diagnosis::new(
                        "Fabric Loader is too old for these mods",
                        format!(
                            "{} need {} of Fabric Loader, but this instance has {have}. Update Fabric Loader in the instance's settings (Loader), or remove those mods.",
                            names.join(", "),
                            c[3].to_owned()
                        ),
                        names,
                    )
                }
            }
        },
    },
    Rule {
        // Fabric's resolver also logs this in English whatever language the game runs in (the quoted
        // "Mod 'X' ... requires" lines above are translated, and then match nothing):
        //  "Reason: [HARD_DEP nuit_interop 1.0.0-beta.5 {depends nuit @ [1.0.0-beta.5]}, HARD_DEP macos_fix 1.16+1.21.11 {depends minecraft @ [<26.1]}]"
        //  "Fix: add [], remove [], replace [[nuit 1.0.0-beta.4] -> add:nuit 1.0.0-beta.5 ([[1.0.0-beta.5,1.0.0-beta.5]])]"
        pattern: r"HARD_DEP \S+ \S+ \{depends \S+ @",
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
        explain: |c, text| broken_mod(&c[1], text),
    },
    Rule {
        pattern: r"Mixin \[[^\]]+\] from mod ([\w\-]+)",
        explain: |c, text| broken_mod(&c[1], text),
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

fn broken_mod(id: &str, text: &str) -> Diagnosis {
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

/// The loader's own suggestion from its English "Fix:" line: "update sodium to 0.8.12".
fn update_hint(text: &str) -> Option<String> {
    let re = Regex::new(r"\[([^\s\[\]]+) [^\s\]]+\] -> add:(\S+) (\S+) ").ok()?;
    let updates: Vec<String> = re
        .captures_iter(text)
        .map(|m| format!("update {} to {}", &m[1], &m[3]))
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

/// What Fabric's English resolver lines say is wrong with the mods (see the rule that calls this).
fn resolution_log(text: &str) -> Diagnosis {
    let mut for_other_minecraft: Vec<String> = Vec::new();
    let mut needs_newer_loader = false;
    if let Ok(re) = Regex::new(r"HARD_DEP (\S+) \S+ \{depends (\S+) @") {
        for m in re.captures_iter(text) {
            match &m[2] {
                "minecraft" if !for_other_minecraft.contains(&m[1].to_owned()) => {
                    for_other_minecraft.push(m[1].to_owned());
                }
                "fabricloader" => needs_newer_loader = true,
                _ => {}
            }
        }
    }
    // The loader's own suggestion: swap a mod for another version of it.
    let mut updates: Vec<String> = Vec::new();
    if let Ok(re) = Regex::new(r"\[([^\s\[\]]+) [^\s\]]+\] -> add:(\S+) (\S+) ") {
        for m in re.captures_iter(text) {
            updates.push(format!("{} to {}", &m[1], &m[3]));
        }
    }
    let have = Regex::new(r"Loading Minecraft (\S+) with")
        .ok()
        .and_then(|re| re.captures(text).map(|c| c[1].to_owned()))
        .unwrap_or_else(|| "this one".to_owned());
    let also = if updates.is_empty() {
        String::new()
    } else {
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
    /// How the player can be reached (their own words, optional); filled in by the app when it sends.
    pub contact: String,
}

/// Lines of the game log's end that are sent.
const REPORT_LOG_LINES: usize = 150;
/// Most of the log's end, and of the crash report's start, that are sent (bytes). All together they
/// stay under what the server keeps (48 KB).
const REPORT_LOG_BYTES: usize = 8 * 1024;
const REPORT_CRASH_BYTES: usize = 16 * 1024;
/// How Java was started: its flags and the classpath's file names.
const REPORT_JAVA_BYTES: usize = 5 * 1024;
/// The launcher's own log (sign-in failures, downloads, launch steps): its last lines.
const REPORT_LAUNCHER_LINES: usize = 80;
const REPORT_LAUNCHER_BYTES: usize = 6 * 1024;

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
        let all = lines.join("\n");
        let skip = lines.len().saturating_sub(REPORT_LOG_LINES);
        let tail = lines[skip..].join("\n");
        let mut text = tail_of(&tail, REPORT_LOG_BYTES).to_owned();
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
            + java.map_or(0, |s| s.len().min(REPORT_JAVA_BYTES))
            + crash_report.map_or(0, |s| s.len().min(REPORT_CRASH_BYTES));
        let evidence_budget = (48 * 1024usize).saturating_sub(text.len() + reserved + 1024);
        let evidence = diagnostic_evidence(&all, evidence_budget);
        if !evidence.is_empty() {
            text.push_str("\n\n---- diagnostics ----\n");
            text.push_str(head_of(&evidence, evidence_budget));
        }
        if !launcher_lines.is_empty() {
            let skip = launcher_lines.len().saturating_sub(REPORT_LAUNCHER_LINES);
            let launcher = launcher_lines[skip..]
                .iter()
                .map(|l| compact_launch(l))
                .collect::<Vec<_>>()
                .join("\n");
            text.push_str("\n\n---- launcher log ----\n");
            text.push_str(tail_of(&launcher, REPORT_LAUNCHER_BYTES));
        }
        if let Some(java) = java {
            text.push_str("\n\n---- java ----\n");
            text.push_str(head_of(java, REPORT_JAVA_BYTES));
        }
        if let Some(report) = crash_report {
            text.push_str("\n\n---- crash report ----\n");
            text.push_str(head_of(report, REPORT_CRASH_BYTES));
        }
        let title = exit_title(title, &all, java.unwrap_or(""));
        Self {
            launcher: env!("CARGO_PKG_VERSION").to_owned(),
            game: game.to_owned(),
            loader: loader.to_owned(),
            os: std::env::consts::OS.to_owned(),
            title: scrub(
                if crash_report.is_none()
                    && diagnose(&all).is_none()
                    && (title.is_empty()
                        || title.starts_with("Game exited with code")
                        || title.starts_with("The game stopped before")
                        || title == "unknown"
                        || title == "The game froze and was closed")
                    && freeze_evidence(&all)
                {
                    "The game froze and was closed"
                } else if orderly_shutdown(&all) && crash_report.is_none() {
                    "Non-crash: Minecraft stopped normally"
                } else {
                    &title
                },
                &Own::this_pc(),
            ),
            log: head_of(&scrub(&text, &Own::this_pc()), 47 * 1024).to_owned(),
            contact: String::new(),
        }
    }
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

/// Only an unrecovered client watchdog stall is evidence of a hang.
pub fn freeze_evidence(text: &str) -> bool {
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
        l.contains("[Arctic diagnostics]") && !l.contains("hang ") && !l.contains("thread ")
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
    use super::*;

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
        assert_eq!(d.title, "2 mod(s) are for another Minecraft version");
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
