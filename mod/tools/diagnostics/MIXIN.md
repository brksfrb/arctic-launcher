Mixin failure context is captured by the Java agent around Fabric's actual
`IMixinTransformer.transformClassBytes` call. Successful transforms add only a
static wrapper call; they do not parse, copy, hash, log, or retain class bytes.
No frame/render hooks were added by this change.

The wrapper rethrows the original Throwable, including when diagnostics cannot
load or write. Cache hits return before this boundary; a transformation failure
therefore records a cache miss, alongside the cache's actual mode/counters.
`input=null` is explicit for generated classes. Method names/descriptors come
from that invocation's input array, not a later resource reread or the partially
modified Mixin ClassNode. Selected configs' `mixinMapping` entries provide the
other target mixins without asking plugins or repeating selection. Unsupported
Mixin internals are reported as unavailable rather than guessed.

Reports distinguish the main JVM thread, Arctic preload threads, and other
threads, with the actual thread name. They include cache state, preloader start
state, successful replay, and attempted/completed plugin callbacks and
shouldApply decisions (including a replay that stopped partway through).

Raw stderr and `arctic-mixin-failure.txt` preserve failures even when the
preloader swallows them or subsequent logging evicts them. The sidecar is next
to the instance-specific hang sidecar, including portal launches. It retains
the first three distinct failed classes plus the latest class, up to 512 KiB;
duplicate failures for a retained class do not rewrite it. The launcher ignores
stale or oversized sidecars. A later fatal class still has its context saved.

The upload starts with a separate full mod inventory (`id version`), collected
before trimming Minecraft's report. It also accepts Fabric's startup list and
the failure sidecar's live resolved metadata, including nested mods. Four-part
version strings are preserved, not scrubbed as IP addresses. Context and other
logs still use the existing personal-data scrubber. Inventories over 24 KiB use
lossless gzip+base64 to leave room within the existing 48 KiB server limit.
To decode that section's second line:

```python
import base64, gzip
print(gzip.decompress(base64.b64decode(encoded_line)).decode())
```

Validation uses the Gradle core tests, Fabric build/tests, `mod/check.py` for
26.3/1.20.1/1.18.2, and the workspace Rust checks. To test a specific Loader's
bytecode guard without starting Minecraft, set
`ARCTIC_DIAGNOSTICS_LOADER_JAR=/path/to/fabric-loader.jar` for the Fabric Gradle
test task. Tests inspect the real transformed delegate and execute a fixture that checks
successful byte identity, no success-side report, swallowed preloader failures,
and original error identity even on diagnostics I/O failure.

For the embedded client pack, `mixin_overlay.py` overlays only ArcticAgent,
MixinReplay, Preloader, and MixinFailureDiagnostics class families onto the
unmerged crash-diagnostics pack. It verifies all other entries in all 32
variants remain byte-identical, including the older clients without this agent.
