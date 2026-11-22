#[cfg(windows)]
mod windows;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verification {
    Official,
    Unsigned,
    Untrusted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Banner {
    Unsigned,
    Untrusted,
}

fn banner(verification: Option<Verification>, release: bool, dismissed: bool) -> Option<Banner> {
    match verification {
        Some(Verification::Untrusted) => Some(Banner::Untrusted),
        Some(Verification::Unsigned) if release && !dismissed => Some(Banner::Unsigned),
        _ => None,
    }
}

#[cfg(windows)]
pub(crate) use windows::BuildSignature;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_and_unchecked_builds_are_quiet() {
        assert_eq!(banner(None, true, false), None);
        assert_eq!(banner(Some(Verification::Official), true, false), None);
    }

    #[test]
    fn unsigned_banner_is_for_release_builds_until_dismissed() {
        let unsigned = Some(Verification::Unsigned);
        assert_eq!(banner(unsigned, true, false), Some(Banner::Unsigned));
        assert_eq!(banner(unsigned, true, true), None);
        assert_eq!(banner(unsigned, false, false), None);
    }

    #[test]
    fn untrusted_signatures_always_warn() {
        for release in [true, false] {
            for dismissed in [true, false] {
                assert_eq!(
                    banner(Some(Verification::Untrusted), release, dismissed),
                    Some(Banner::Untrusted)
                );
            }
        }
    }
}
