//! What a starting game is doing before its window opens, read from its log.

/// Stages in the order they happen; a game only moves forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// Java is starting (nothing logged yet).
    Java,
    /// The mod loader found this many mods.
    Mods(u32),
    /// Minecraft sets itself up (the long quiet part with mods).
    Game,
    /// Signed in, the window is being created.
    Window,
}

impl Stage {
    pub fn label(self) -> String {
        match self {
            Stage::Java => "Starting Java".into(),
            Stage::Mods(1) => "Loading 1 mod".into(),
            Stage::Mods(n) => format!("Loading {n} mods"),
            Stage::Game => "Preparing the game".into(),
            Stage::Window => "Opening the window".into(),
        }
    }

    fn rank(self) -> u8 {
        match self {
            Stage::Java => 0,
            Stage::Mods(_) => 1,
            Stage::Game => 2,
            Stage::Window => 3,
        }
    }

    /// The stage after this log line, if it moves things forward.
    pub fn advance(self, line: &str) -> Stage {
        let next = if line.contains("Setting user:") {
            Some(Stage::Window)
        } else if line.contains("SpongePowered MIXIN") || line.contains("Datafixer") {
            Some(Stage::Game)
        } else {
            mods_count(line).map(Stage::Mods)
        };
        match next {
            Some(n) if n.rank() > self.rank() => n,
            _ => self,
        }
    }
}

/// "Loading 57 mods:" (Fabric/Quilt) → 57.
fn mods_count(line: &str) -> Option<u32> {
    let rest = &line[line.find("Loading ")? + "Loading ".len()..];
    let (n, tail) = rest.split_once(' ')?;
    (tail.starts_with("mods") || tail.starts_with("mod:")).then(|| n.parse().ok())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_a_fabric_startup() {
        let log = [
            "[17:04:11] [main/INFO]: Loading Minecraft 26.2 with Fabric Loader 0.19.5",
            "[17:04:11] [main/INFO]: Loading 57 mods:",
            "\t- sodium 0.9.2+mc26.2",
            "[17:04:12] [main/INFO]: SpongePowered MIXIN Subsystem Version=0.8.7",
            "[17:04:16] [Datafixer Bootstrap #0/INFO]: 295 Datafixer optimizations took 341 milliseconds",
            "[17:04:25] [Render thread/INFO]: Setting user: brksfrb2",
        ];
        let stages: Vec<Stage> = log
            .iter()
            .scan(Stage::Java, |s, l| {
                *s = s.advance(l);
                Some(*s)
            })
            .collect();
        assert_eq!(
            stages,
            [
                Stage::Java,
                Stage::Mods(57),
                Stage::Mods(57),
                Stage::Game,
                Stage::Game,
                Stage::Window
            ]
        );
        assert_eq!(Stage::Mods(57).label(), "Loading 57 mods");
    }

    #[test]
    fn never_goes_back() {
        assert_eq!(Stage::Window.advance("Loading 3 mods:"), Stage::Window);
        assert_eq!(Stage::Game.advance("Loading 3 mods:"), Stage::Game);
    }

    #[test]
    fn vanilla_goes_straight_to_the_window() {
        assert_eq!(
            Stage::Java.advance("[12:00:00] [Render thread/INFO]: Setting user: Steve"),
            Stage::Window
        );
        assert_eq!(Stage::Java.advance("Loading chunks"), Stage::Java);
    }
}
