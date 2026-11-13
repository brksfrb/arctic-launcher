# Replay protocol handovers

Base: fetched `origin/main`, `b4564fae6fe55ef4bc34e4bdbc8c646074996dfb`.
No unmerged replay branch is included in this branch.

## Findings and limits

`ReplayPlayback` previously created a bare `EmbeddedChannel(connection)`. On
1.20.5+, Minecraft writes inbound/outbound protocol configuration tasks through
its channel. The bare channel completes those writes without executing the
tasks; playback subsequently discards them with its outgoing packets. Its
manual decoder also bypasses the terminal-packet barrier in `PacketDecoder`.
The transport now uses Minecraft's in-memory pipeline and reproduces that
barrier when decoding outside the pipeline, including retiring the old play
bundler before the next play phase installs one.

Separately, any exception thrown by a configuration/login handler was swallowed
by `ReplayViewer.apply`. A failed finish-configuration handler leaves the
connection in configuration; later play bytes then fail using that old codec.
The `problem == null` log guard never changed `problem`, so it logged every
failure and never stopped feeding. Playback now verifies each handover, stops
immediately when one fails, and stops after 16 consecutive ordinary packet
failures. The first failure includes the Minecraft protocol number, phase,
packet id and cause. A success resets the consecutive-failure counter.

These defects are established from code and regression tests. The first fault
in crash group 270 is **not established** by the supplied excerpt: no original
recording or log preceding packet 22507 was supplied. In particular, the tests
cannot prove that this recording's finish-configuration handler failed, or why.

Recording still encodes packets at connection-read time using that connection's
inbound protocol, before its listener handles the packet. If a phase-boundary
packet cannot be encoded, the take now ends at its last valid packet instead
of silently omitting a boundary and saving later packets in the wrong phase.
The file format is unchanged; existing files remain readable. Transfer packets
remain suppressed in playback: real transfers create a new connection/take,
whereas proxy reconfiguration on the same connection is replayed in place.

## Regression tests

`ReplayProtocolTest` uses real Minecraft codecs and a real `Connection` with
lightweight listener proxies. It encodes and opens an existing-format `.mcpr`,
then decodes login, known-packs (1.20.5+), registry data, enabled features,
finish-configuration, and three repeated start-configuration/configuration/play
cycles. The listeners install protocols and send real outgoing acknowledgements
in Minecraft's order. The test checks that configuration tasks execute and the
final play packet is handled. It also checks an incomplete handover and an
unencodable recording boundary. It does not construct a rendered world or run
Minecraft's registry resource-loading handler.

`ReplayViewerFailureTest` covers join, normal playback, indexed/unindexed seeks,
success resetting the error count, fatal handovers, a dismissed problem
never restarting packet feeding, and closing/releasing a replay that fails
before its world or HUD exists.

Run from the repository root:

```sh
bash mod/gradlew -p mod/core build --offline --no-daemon --max-workers=2
bash mod/gradlew -p mod/versions/fabric build -Pminecraft_version=1.21.11 --offline --no-daemon --max-workers=2
bash mod/gradlew -p mod/versions/fabric build -Pminecraft_version=26.3 --offline --no-daemon --max-workers=2
JAVA_HOME=/usr/lib/jvm/java-25-openjdk-amd64 python mod/versions/fabric/mixcheck.py 1.21.11 26.3
```

The same version-preprocessed tests run on every supported 1.20.2+ build target.
No game, server, production container, or customer process is started by them.

## Validation on this branch

- Core: 246 tests, no failures.
- All 26 Fabric targets in `mod/targets.json`: Gradle build and mixin check passed.
- Final protocol regression builds: 1.20.2, 1.20.6, 1.21.11 and 26.3.
- File-level three-way compatibility checks against
  `origin/fix/replay-null-renderer` (`a486947a`),
  `origin/fix/replay-load-speed` (`918b74c9`) and
  `origin/fix/replay-record-in-world` (`ba370083`) found no conflicts.
  Shared fatal-error handling matches the existing load-speed branch.
- No Rust files changed, games started, or production processes modified.
