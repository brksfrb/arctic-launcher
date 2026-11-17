# Opt-in HUD CPU timing

`python mod/tools/hudbench/build.py [path/to/asm-9.9.jar]` makes `build/agent.jar`
and `build/meter.jar` (requires JDK 21+). Nothing is included in the mod jar.

Add `meter.jar` as a development `runtimeOnly files(...)` dependency so Fabric
can expose it to the game's class loader. Add this VM argument, using absolute
paths: `-javaagent:/path/agent.jar=/path/meter.jar`.

The agent instruments only `Hud.render`, with a timestamp at entry and return.
After 20 seconds of HUD rendering it retains the last 1,000 calls and reports
mean/median/p95 CPU submission times at shutdown. It does not flush the GPU,
measure GPU time, or time replay capture and the rest of the mod. No timings
are collected unless the separate agent is explicitly supplied.

Use the same scene, HUD slots, GUI scale, replay settings and JVM for both jars.
The shared CPU allocation bench is `mod/gradlew -p mod/core test --tests
'*HudCostTest'`, with a Gradle init script setting the test JVM system property
`arctic.hud.bench=true` and `testLogging.showStandardStreams=true`.
