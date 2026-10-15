# Test checklist, 2026-10-15 batch

Needs a build with the new client pack (commits `02a9f95` to `d4b7d00` on `codex/live-gameplay-video`).
Test on **26.3** and **1.8.9** at least; add 1.21.x and 1.16.5 if there's time.
For each item: works / doesn't, the Minecraft version, and a screenshot or the log line if it fails.

## A. In game (Arctic Client)

| # | Do | Expect |
|---|---|---|
| A1 | Multiplayer → Add/Edit server. Type an address, press Left/Right/Home/End, Shift+arrows, drag with the mouse, Ctrl+A, Ctrl+C, Ctrl+X, Ctrl+V | A real cursor moves, text selects, copy/cut/paste work. A "Copy" button sits next to the address |
| A2 | Replays tab (Right Shift menu): turn the "Replay" switch off, join a world, press F9 | Nothing is recorded; "Replay is off" note shown. Turn it on, rejoin: F9 saves again |
| A3 | Replays tab → "Open videos folder" | The folder opens in Explorer / Finder / file manager (no pop-up with a path) |
| A4 | Join a server, get kicked or cancel the join, then use Reconnect | It reconnects to **that** server, not the last one you played |
| A5 | Auto Reconnect settings: set 0.1 s, then 1 s; disconnect | The countdown is in ms / s and the tries limit is respected |
| A6 | Cancel a join in progress, or Disconnect from a server | You land on the server list, not the title screen; no frame of the plain vanilla list first |
| A7 | Zoom: set sensitivity to 50% and 150% (stepper in the Zoom card). Do it on 1.8.9 too | Mouse turns slower / faster while zoomed, same feel on 1.8.9 and 26.3 |
| A8 | HUD editor: drag a widget onto the middle bar to remove it, then add it back | It comes back somewhere you can grab it, not under the bar |
| A9 | Right Shift → search "potions" | The Effects widget shows up |
| A10 | Take a screenshot (F2) | The pop-up appears at once (well under a second); button says "Copying..." then "Copied" |
| A11 | Die in singleplayer world "New World", then open another world also called "New World" | No death waypoint / "Last death" from the other world |

## B. Chat (new)

| # | Do | Expect |
|---|---|---|
| B1 | Arctic menu → Chat History. Chat a few lines, search for a word | Lines are found, newest first, with times |
| B2 | Type `diamond, trade, -spam` in the search | Lines with diamond or trade, without spam |
| B3 | "Save as a filter", then HUD editor → add the "Filtered chat" widget, "Show" the filter | A small second chat on screen showing only matching lines |
| B4 | Chat History → "Chat keeps": set 10000; paste/spam > 150 lines in chat, scroll up with the wheel / PageUp | You can scroll back past line 100 (vanilla stops at 100) |
| B5 | Send the same line 10 times fast | One line with (x10); no stutter |
| B6 | Emoji and Symbols page: click a symbol, paste in chat with Ctrl+V. Also type `:heart: :star:` and send | Symbols appear (not boxes). On servers that filter symbols, note which |
| B7 | Join another server, open Chat History | Lines from the first server are still there |

## C. Packs (new)

| # | Do | Expect |
|---|---|---|
| C1 | Launcher → Instances → Packs: Group menu on a pack → "New group…" → name → Enter | Pack appears under the group name |
| C2 | Open the game → Packs & Shaders | Same groups as chips + "[group]" tags; Group button changes it; the launcher shows the change |
| C3 | Same on Shaders | Same behaviour |

## D. Launcher

| # | Do | Expect |
|---|---|---|
| D1 | Play tab: gear next to the account chip | Opens that instance's settings |
| D2 | Settings → Appearance → Background → "Your picture…" | File picker; the picture becomes the backdrop |
| D3 | Settings → "Desktop shortcut" | A shortcut appears on the desktop and launches the launcher |
| D4 | Offline account with a skin chosen in Cosmetics | The launcher avatar shows that skin's face, not Steve |
| D5 | Microsoft sign-in page | A styled "You're signed in" page, not a bare line |
| D6 | Screenshots tab: click a screenshot, close it | It grows out of its cell and shrinks back |
| D7 | Instances → a custom Fabric 1.8.9 instance → Mods → search "OptiFabric" | "Legacy OptiFabric" is listed |
| D8 | Instance → Replays page | A "Gameplay videos" card with Play / Show in folder / Trash |
| D9 | Logs tab: drag-select text to the bottom edge of the log | The log scrolls while you drag; auto-scroll doesn't snap back |
| D10 | Vanilla running 26.2, press Play for 1.21.11 | A clear message "Vanilla is already running 26.2…", no file-in-use error |
| D11 | Import world: "From other instances" | Instances of other profiles are listed too |
| D12 | Set entity distance 200% in a 26.x instance, play a 1.8.9 instance, back to 26.x | Still 200% |
| D13 | Instance with Forge: open its page | A card says the Arctic Client isn't on Forge yet |

## E. Cosmetics (the bug you reported)

Two players on the **Flash Hosting** server, same version, both Arctic Client:
1. Each opens Arctic menu → **Cosmetics Check** and sends me what it shows (all lines).
2. Each looks at the other; say whether skin, cape, and 3D cosmetics show.
3. Press "Check in again now", wait 20 s, look again.
Note: whether the friend is on a Vanilla or Fabric instance, any mods (Iris, Sodium...) and the
Minecraft version. The server fix for offline UUID collisions (`4443a39`) is **not deployed yet**:
test before and after the deploy if possible.

## What to send back
- Which item numbers fail, the Minecraft version, and `latest.log` (game folder → logs) right
  after the failure.
- For anything visual: a screenshot.
