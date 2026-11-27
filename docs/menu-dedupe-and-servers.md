# Mods and multiplayer list corrections

Scoreboard's visual feature and vanilla HUD part now share one card. Held Item's
hand transform and HUD readout also share one card: their translations collided
in French, Portuguese, Italian, Dutch, Polish, Russian, Ukrainian, Chinese and
Korean. Both settings forms, independent visibility controls, HUD appearance,
live preview and Edit HUD remain available. Existing configuration keys and
positions are retained. A regression test checks visible names in all 14 locales
across all combinations of feature, screen hook, scoreboard tweak and movable
vanilla HUD capabilities; other version capabilities are enabled during the test.

The Java multiplayer list previously measured one ping/pong round trip, already
excluding DNS, TCP connection, handshake and status JSON. It now takes the lowest
valid RTT from up to three fresh status connections (servers can close after one
pong), validating the echoed payload. Failure of a later sample retains the first
status. Missing/invalid pongs have unknown latency rather than a fabricated RTT.
All Java version adapters use this shared implementation.

The Rust launcher also already excluded connection and handshake time. Its status
socket now enables TCP_NODELAY, avoiding small-packet batching, and rejects wrong
pong payloads. This removes a possible measurement artifact; neither change can
guarantee Lunar's 42 ms for a server observed at 60 ms. Routing, server load and
sampling time still matter. Loopback tests insert a 500 ms status delay and verify
it is excluded from displayed latency; Java also verifies minimum sample selection.

Saved lists in Java and Rust use address identity without network lookups: trim
whitespace, fold hostname case, strip trailing DNS dots, and equate omitted and
explicit default ports (including bracketed IPv6). Distinct nondefault ports stay
separate. The first visible entry retains its name and metadata. Java deduplicates in
memory on load and persists that list on the next save, preserving hidden entries;
edits, reordering and deletion therefore cannot reveal a stale duplicate. Rust
filters duplicate rows on read. New duplicate additions
are ignored. Direct Connection has Add to list beside Copy in all 14 locales.

Glass fix 76e99c5e is already on origin/main as squash commit 9818c437 (PR #61).
Its cherry-pick is empty; no duplicate fix commit is needed. The core glass
regression tests and the adapter resource-pack tests exercise the existing fix.

Validation: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
and `cargo test --workspace` passed (711 Rust tests passed, three ignored).
Shared Java core Gradle tests passed (362 tests, zero failures, two skipped),
including locale/capability combinations and original HUD key bindings. The i18n
checker and seven CTM adapter API variants passed. Builds used four Cargo jobs,
four Gradle workers and a 3 GB Gradle heap. No live Lunar comparison was performed.
