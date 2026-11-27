# Startup measurement

Isolated data directory, offline test account, one game at a time, nothing building while a
game starts. Never point these at a real launcher data directory.

1. **Launcher (Play to Java process).** The `startup_bench` example runs the launcher's real
   `prepare` (file checks, downloads, loader, Arctic Client and performance-mod sync) and
   optionally spawns Java:

   ```sh
   cargo run --release -p arctic-core --features offline-accounts --example startup_bench -- \
       <data-root> 1.16.5 --performance --command-out cmd.json [--spawn]
   ```

   The first run in a fresh root downloads everything; later runs measure the warm path.
   The launcher log has one `ready to start in …ms: <step> <ms>, …` line and
   `[startup] <stage>: checked N files in … ms` per download stage.

2. **Game start.** `run.py cmd.json <game-dir> <label> --jar arctic.jar [--pack stress]`
   (or `--no-arctic`) starts the planned command under Xvfb with Mesa's software GL, waits for
   ModernFix's `Game took N seconds` line (the number players report; `--performance` puts
   ModernFix in on versions it supports) and prints one `RESULT` line, with the count of
   CTM early-stage errors and `space` font warnings.

3. **Stress pack.** `stresspack.py <client.jar> <game-dir>/resourcepacks/stress <pack_format>`
   upscales every vanilla block texture to 128 px (animations kept), a stand-in for a large
   HD pack. It is not the reporting player's packs.

The Arctic Client logs `[startup] <step>: N ms` for each init step and
`[startup] CTM pack rules … / CTM atlas bound: N source sprites, 47 tiles per face` per
atlas reload. A 1.16.5 run with Fabric API and ModernFix must show no
`was not loaded in early stage` errors.
