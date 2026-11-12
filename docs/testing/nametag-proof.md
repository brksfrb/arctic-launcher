# Two-player nametag proof

Build the mods (`JAVA_HOME` must be JDK 25), then run
`python3 mod/tools/check_badge_assets.py` to check both the legacy pictures
and modern bitmap-font resources in every built jar. On 1.21.6 and newer,
these fonts must also be in the required `resourcepacks/client` pack:
Fabric Loader alone does not publish mod assets to Minecraft, and Arctic
must show its badge even without Fabric Resource Loader / performance mods.

For the runtime check, start an offline server bound to loopback and two
Arctic clients named `ArcticDev` and `ArcticDev2`, in separate game folders.
Use the built jars. The test deliberately works with Arctic alone (without
Fabric API), covering the standalone resource-loading path too.

Give each client these JVM properties, with its own absolute command path:

```
-Darctic.selftest=nametagproof
-Darctic.proof.commands=/absolute/game-folder/command.txt
```

The opt-in test seeds only those two offline UUIDs into the normal Arctic
membership cache, because offline accounts cannot authenticate to the
production membership service. It does not replace badge drawing or the
camera transforms. It closes menus and prevents pausing on focus loss.

Put the players in creative mode on a loaded platform. Teleport the observer
to `0 100 0 0 0`, the second player to `0 100 4 180 0`. Write `normal` to the
observer's command file, wait a few seconds, then write `shot`. The command
file is consumed once per second; wait for each command's log before sending
the next. Screenshots go to the normal game screenshots directory.

Write `freelook`, wait, then `rotate`, wait. This holds the same simulated
Freelook key state as the existing world self-test and supplies 600 units of
mouse movement (90 degrees). Move the second player into the new camera
view, e.g. `tp ArcticDev2 -2 100 2 90 0`, then send `shot` again. The log must
say `freelook=true cameraYaw=90.0 body=0.0`; the second player's nametag must
remain readable and show the white snowflake. Inspect both PNGs, rather
than accepting only the camera-angle log. Repeat on 1.8.9, 1.21.11 and 26.3.

The server can move the second player to put its name in each view, but must
not rotate the observer's body after the normal screenshot. This tests
another real network player's name, rather than an own-name or named-mob
substitute. Stop both clients and the server after capturing the evidence.
