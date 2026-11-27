# Join pass harness

Measures connect-to-in-game time and the first 60 s of frames after joining, on 1.8.9, 1.21.11 and 26.3,
under Xvfb with Mesa (llvmpipe; `--noop` uses the no-op GPU on 1.8.9, which crashes Mesa on 1.21+).

- `agent/` - a Java agent (Java 8 bytecode; ASM comes from Fabric Loader's classpath). It hooks the buffer swap
  (`Display.update`, `glfwSwapBuffers`, `SDL_GL_SwapWindow`) for frame times and client-thread CPU, the game's
  set-screen method for the join timeline, samples the client thread's stack every 5 ms, and joins by itself:
  a server (`mp:host:port`, resource packs accepted) or a saved world (`sp:folder`) once the title screen has
  been up for a while. Build: `javac --release 8 -cp asm.jar` J.java into joinclock.jar (bootstrap) and
  Agent.java into joinagent.jar (Premain-Class joinagent.Agent), side by side.
- `run.py <189|1211|263> <label> [--arctic JAR] [--polarium JAR] [--mode mp|sp] [--perf] [--noop] [--pack]` -
  local offline vanilla server with 80 bot players (`bots.js`, minecraft-protocol; through ViaProxy on 26.3),
  a glass lobby with 200 entities, a fake Arctic looks server, then one client start.
- `analyze.py`, `summary.py <prefix>` - join ms, first-60 s FPS, 1% lows, time to stable, client CPU, and
  "Java ms/frame": client-thread time outside GL/driver calls (what a real GPU still pays; llvmpipe makes
  1.21+ draw-bound, so FPS alone hides CPU-side work). `diffprof.py A B from to` diffs two profiles.

Paths are this machine's (`/home/claude/joinpass-work`, the launcher's dry-run argument files).
- `--lat MS` delays every fake looks-server answer (6000 ms showed no Arctic request on the join path:
  joins took as long as with 150 ms). `batch-final.sh` is the interleaved base/final A/B (client -Xmx2G, one
  server + client at a time).
