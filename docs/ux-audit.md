# Arctic Launcher UX audit (Phase 1: findings only)

Audit date: 2026-11-08. Scope: `crates/arctic-app` (the egui launcher) and the core flows it drives in `crates/arctic-core` (install, launch, accounts, mods, versions, settings, screenshots, replays, updates, storage, migration, skins, servers, friends, Play together, voice, Bedrock). Every finding was confirmed by reading the code at the cited lines; quoted strings are verbatim.

Ranking: impact first (high, med, low), then effort (S before M before L). "Changes behavior" marks fixes that would change existing user-facing behavior in a way a user might not expect.

Totals: 105 findings. 26 high, 43 medium, 36 low. 29 change behavior.

## Summary table

| # | Title | Impact | Effort | Changes behavior |
|---|---|---|---|---|
| 1 | Force close is reported to the player as a crash | high | S | yes |
| 2 | Toasts that carry an action or a long hint vanish before they can be read | high | S | yes |
| 3 | Version-list failure reads "Loading versions…" forever on custom instances | high | S | no |
| 4 | "Skip setup" opens the migration wizard | high | S | yes |
| 5 | Linux players can pick the tray in setup but never change it later | high | S | no |
| 6 | Play together's "Launch Minecraft" doesn't join the world it just connected to | high | S | yes |
| 7 | Friends' Join buttons bypass `join_server` | high | S | yes |
| 8 | "Remove" on a mod deletes the jar permanently with no confirm and no undo | high | S | yes |
| 9 | "Delete" on a resource or shader pack is permanent, no confirm | high | S | yes |
| 10 | Removing a friend or declining a request is one irreversible click | high | S | yes |
| 11 | Deleting a library skin: no confirm, file deleted, no undo | high | S | yes |
| 12 | A chat message is lost when sending fails | high | S | no |
| 13 | Voice room join failures are swallowed and the status says all is well | high | S | no |
| 14 | Modpack import fails on long pack names and makes duplicate-named instances | high | S | yes |
| 15 | An unplugged storage drive shows red text with no action, and only in Settings | high | S | no |
| 16 | "Launch failed" is a raw error chain with no action, and never reaches the run's log | high | M | no |
| 17 | An expired Microsoft session becomes a generic launch failure with no "sign in again" | high | M | yes |
| 18 | Sign-in failures are double-prefixed, raw, and offer no next step | high | M | yes |
| 19 | Browser sign-in is a silent five-minute wait with no fallback | high | M | no |
| 20 | A launch cannot be cancelled while it downloads | high | M | yes |
| 21 | Trashed worlds can never be brought back from the UI | high | M | no |
| 22 | There is no way to remove a server from the Java server list | high | M | no |
| 23 | Migration has no cancel, and the dialog can't be closed while scanning | high | M | no |
| 24 | Moving the storage folder is uncancellable and ends with an unannounced restart | high | M | yes |
| 25 | Import from another launcher: one locked file aborts the whole instance, with a raw I/O error | high | M | yes |
| 26 | Video export on Linux and macOS dead-ends with "install it yourself" | high | L | yes |
| 27 | The tray host and the hidden start read a settings file that no longer exists | med | S | yes |
| 28 | "Restore defaults" has no confirmation and wipes more than settings | med | S | yes |
| 29 | The Network section needs Save while the page header promises autosave | med | S | no |
| 30 | Turning the tray setting off leaves the icon until restart | med | S | no |
| 31 | The Settings memory slider lacks the guidance onboarding gives | med | S | no |
| 32 | Update status and the manual check live in About, and a failed startup check is invisible | med | S | no |
| 33 | "Game settings for new instances" is filed under "Data" | med | S | no |
| 34 | An unknown crash is explained as "Exit code 1." | med | S | no |
| 35 | Snapshots and old versions hide behind a Settings toggle; the picker gives no hint | med | S | no |
| 36 | Quick actions are unlabeled squares, and the two "log" actions behave differently | med | S | yes |
| 37 | No guidance when a game hangs before its window appears | med | S | no |
| 38 | A failed modpack install leaves a never-seen instance in the Trash | med | S | no |
| 39 | The create-instance form accepts a name it will then reject | med | S | no |
| 40 | Cancelling the world-zip picker shows a green "No file chosen" success toast | med | S | no |
| 41 | Partial world-import failures are swallowed | med | S | no |
| 42 | "N could not be updated." never says which mods | med | S | no |
| 43 | Resource-pack search is capped at 20 results with no "load more" | med | S | no |
| 44 | Load and search errors are dead-end labels with no Retry, in ten places | med | S | no |
| 45 | "Play this world (1.20+)" is offered on every version and silently does nothing before 1.20 | med | S | yes |
| 46 | The migration "done" screen gives no way to open or play what came over | med | S | no |
| 47 | The replay list goes stale after a game session | med | S | no |
| 48 | A thumbnail that failed to load looks exactly like one still loading | med | S | no |
| 49 | Trash actions have no Undo and show internal folder paths | med | S | no |
| 50 | Moving an instance's game folder shows a fake progress bar and throws the real progress away | med | S | no |
| 51 | One feature, three names: "Move to Arctic", "From another launcher", "Bring over" | med | S | no |
| 52 | "Minecraft skin change failed" fires on plain loading, and the fix it names has no button | med | S | no |
| 53 | Ping failures print the raw transport error under each server | med | S | no |
| 54 | "…are loading." forever when the cosmetics catalog failed, and clicks silently do nothing | med | S | no |
| 55 | Changing the real Mojang skin or cape has no confirmation | med | S | yes |
| 56 | Naming drift: three kinds of "code", two kinds of "Share", tab labels that don't match page titles | med | S | yes |
| 57 | The host card tells you to copy-paste the invite code though one-click Invite exists | med | S | no |
| 58 | Generic toast titles with raw bodies: "Friends", "Gallery", "Bedrock" | med | S | no |
| 59 | The Play together error is raw, sticky and cannot be dismissed | med | S | no |
| 60 | The proxy warning is an Error toast on every single launch | med | S | yes |
| 61 | Four user-facing strings contain runs of 15-22 literal spaces | med | S | no |
| 62 | Switching account leaves the Play tab | med | M | no |
| 63 | Join while the game is starting tells the player to come back and click again | med | M | yes |
| 64 | Clash fixes expose raw mod ids and may install the wrong Modrinth project | med | M | no |
| 65 | Hand-added mods show as file names forever though recognition exists | med | M | no |
| 66 | Installed packs are listed by raw file name and search never marks "Installed" | med | M | no |
| 67 | The result of a version change or loader switch is crammed into a 4.5-second toast | med | M | no |
| 68 | Importing a share code or profile has no Cancel and locks the dialog | med | M | no |
| 69 | Long instance tasks have no Cancel and some have no progress | med | L | no |
| 70 | "Later" on the update banner lasts one session only | low | S | yes |
| 71 | Restart and update failures show raw I/O text and internal file names | low | S | no |
| 72 | Button vocabulary is inconsistent across dialogs | low | S | no |
| 73 | The Accounts tab's empty state talks about accounts that don't exist | low | S | no |
| 74 | Tray "Play" and `--launch` dead-end without an account or with a bad version id | low | S | no |
| 75 | "Data folder" in About is a plain string | low | S | no |
| 76 | Profile removal exposes an internal path and offers no restore | low | S | no |
| 77 | Disabled Microsoft tiles give no pointer, and a dropped-in client id needs a restart | low | S | no |
| 78 | "Try again" after an expired device code goes back to the method chooser | low | S | no |
| 79 | Messages point to a place in the UI instead of offering a button | low | S | no |
| 80 | "last today" / "last yesterday" | low | S | no |
| 81 | Two "ago" dialects on the same tab | low | S | no |
| 82 | The version picker's Installed tab says "No matching versions" when nothing is installed | low | S | no |
| 83 | Version dates are raw ISO | low | S | no |
| 84 | Profile-load errors at startup are raw I/O text with no action | low | S | no |
| 85 | Modpack version rows show raw Modrinth ids | low | S | no |
| 86 | World backup opens a file manager unasked and names the zip in the toast | low | S | yes |
| 87 | Mod update checks are manual only | low | S | yes |
| 88 | The Java path is a text box with a Windows-only hint and no Browse | low | S | no |
| 89 | "Remove", "Delete" and "Move to trash" are used interchangeably | low | S | no |
| 90 | World rows lead with the on-disk folder name | low | S | no |
| 91 | The loader build picker shows raw build ids and needs a manual pick to update | low | S | no |
| 92 | "0.0 GB to copy", full source paths and a "Folder:" prefix in the migration chooser | low | S | no |
| 93 | Picking a non-empty storage folder asks for two confirmations | low | S | no |
| 94 | Unfinished replay recordings are deleted silently at every start | low | S | yes |
| 95 | "Couldn't" and "Could not" are both used, about evenly | low | S | no |
| 96 | No way to save a log or open a crash report | low | S | no |
| 97 | Screenshots: redundant Refresh, misleading filtered-empty state, no folder access when empty | low | S | no |
| 98 | Migration excludes screenshots by default without saying so | low | S | no |
| 99 | Own capes are named "Your cape 3" and renumber themselves | low | S | no |
| 100 | Voice "couldn't start" is raw, retries silently, and the status is buried | low | S | no |
| 101 | Discover: "Add an account first." has no action, and the Premium filter is dead code | low | S | no |
| 102 | Bedrock world rename drops the edit when focus leaves the field | low | S | yes |
| 103 | The friend code is shown twice on the same page | low | S | no |
| 104 | Keeping an uploaded cape can fail silently | low | S | no |
| 105 | "Play another copy" makes the player switch accounts by hand first | low | M | no |

## Recurring patterns

These themes explain most of the list and are worth fixing once, centrally:

- **Errors are `e.to_string()` with no action.** Core errors render as "I/O error at <full path>: <os error>", "network error: …", "authentication failed: …". Toasts and labels pass them straight through, almost never with a Retry, "View logs", "Sign in" or "Open folder" button (findings 16, 17, 18, 25, 44, 53, 58, 71, 84, 95). A single error-to-plain-language mapper plus a shared error row with Retry would fix most of them.
- **Toasts are the only channel, and they expire.** Crash fixes, retarget results, memory warnings and multi-sentence hints all live in a 4.5 s or 9 s toast with no history (2, 15, 60, 67).
- **Destructive actions without confirm or undo.** Mods, packs, skins, friends, worlds, replays, screenshots: some delete outright, some trash with no way back, and the verbs don't say which (8, 9, 10, 11, 21, 49, 89).
- **Long tasks without Cancel.** Launch downloads, modpack install, migration, storage move, share import, duplicate (20, 23, 24, 50, 68, 69).
- **Raw internals in user text.** Trash folder paths, exit codes, build ids, jar file names, Modrinth facet ids, ISO dates, ports (21, 34, 49, 64, 65, 66, 83, 85, 90, 91, 92).
- **Wording drift.** "Couldn't" vs "Could not", "Remove" vs "Delete" vs "Move to trash", three kinds of "code", tab labels that don't match page titles, Yes/No buttons next to verb buttons (51, 56, 72, 80, 81, 89, 95).

## Findings

## High impact

### 1. Force close is reported to the player as a crash
- **Where:** `crates/arctic-app/src/runs.rs:542-572`, `crates/arctic-core/src/launch/process.rs:139-141`, `crates/arctic-core/src/launch/process.rs:166-186`
- **Now:** Force close calls `child.kill()`. Windows reports exit code 1 and Unix reports 128+9, so `report_exit` takes the `Some(c)` branch: an Error toast "{title} closed unexpectedly" / "Exit code 1.", the run is marked crashed, a crash report is offered ("Help fix this crash?") and a stability record is attempted. Nothing on the run remembers that the player asked for the kill. Only an exit with no code at all reaches "{title} was stopped".
- **Fix:** Record the kill request on the run (or have the watcher report `killed: true`) and route it to the "was stopped" branch, skipping diagnosis and the report offer.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 2. Toasts that carry an action or a long hint vanish before they can be read
- **Where:** `crates/arctic-app/src/toasts.rs:24-30`, `crates/arctic-app/src/runs.rs:514-517`, `crates/arctic-app/src/runs.rs:592-597`, `crates/arctic-app/src/app.rs:1110-1118`
- **Now:** Every toast auto-dismisses: Info and Success after 4.5 s, Error after 9 s, and they do not pause on hover. "Help fix this crash?" (Info, with "Send report") lives 4.5 s. The crash diagnosis with the one-click "Fix it" lives 9 s and is offered nowhere else afterwards. "This may be a problem with your PC" carries four sentences about MemTest86 in 4.5 s. "{title}: not much memory" has no "Open memory settings". Players read the game's crash screen first and return to an empty launcher.
- **Fix:** Toasts with an action, or an Error kind, stay until dismissed (or at least pause while hovered). Re-surface the last run's diagnosis and Fix button in the Play tab status area.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 3. Version-list failure reads "Loading versions…" forever on custom instances
- **Where:** `crates/arctic-app/src/ui/play.rs:644-645`, `crates/arctic-app/src/ui/play.rs:80-88`, `crates/arctic-app/src/ui/play.rs:345-350`
- **Now:** `blocked_reason` returns "Loading versions…" for any manifest state that is not Ready, including Failed. The "Could not load versions: {e}" label and Retry button exist only inside `version_picker`, which is drawn only for the default Vanilla instance. A player on a custom instance with no cached manifest sees a disabled PLAY and "Loading versions…" indefinitely. Even on Vanilla the text is the raw error ("network error: … timeout …").
- **Fix:** Distinguish Failed in `blocked_reason` ("Couldn't load the version list. Check your connection.") and show Retry on the Play tab for every instance.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** no

### 4. "Skip setup" opens the migration wizard
- **Where:** `crates/arctic-app/src/ui/onboarding.rs:143-147`, `crates/arctic-app/src/ui/onboarding.rs:192-195`, `crates/arctic-app/src/ui/onboarding.rs:643-646`
- **Now:** When another launcher is detected, `first` starts as `FirstPlay::Move`. Clicking "Skip setup" on the Welcome screen calls `finish_onboarding`, which reads `first` (still Move) and opens the Instances tab plus the "Move to Arctic" dialog. A player who explicitly skipped setup lands in an import wizard.
- **Fix:** When `step == Welcome`, treat `first` as `Later` (the same guard already used for the desktop shortcut at line 623).
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 5. Linux players can pick the tray in setup but never change it later
- **Where:** `crates/arctic-app/src/ui/settings.rs:109-112`, `crates/arctic-app/src/ui/onboarding.rs:298`, `crates/arctic-app/src/tray.rs:125-224`
- **Now:** Onboarding offers "Keep Arctic in the system tray" on Windows and Linux, and `tray.rs` has a full Linux implementation, but the Settings checkbox "Keep Arctic in the system tray when closed" is wrapped in `if cfg!(windows)`. The default is `tray: true`, so a Linux player has no way to turn it off.
- **Fix:** Use `cfg!(any(windows, target_os = "linux"))` in Settings too.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** no

### 6. Play together's "Launch Minecraft" doesn't join the world it just connected to
- **Where:** `crates/arctic-app/src/ui/together.rs:223-249`
- **Now:** After connecting, the card lists manual steps ("Start Minecraft (same version as your friend).", "Open Multiplayer. The world is listed under the LAN section.", "Or Direct Connect to localhost:{port}") with a raw port number and a Copy button. "Launch Minecraft" calls `launch_selected()`, a plain launch, even though `launch_into(QuickPlay::Server(..))` exists and is used elsewhere.
- **Fix:** Have the button call `join_server("localhost:{port}")`, which reuses a running game or launches straight into the world. Drop the step list; keep "Copy address" as a small fallback.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 7. Friends' Join buttons bypass `join_server`
- **Where:** `crates/arctic-app/src/ui/friends.rs:515`, `crates/arctic-app/src/ui/friends.rs:632-638`, `crates/arctic-app/src/app.rs:641-689`
- **Now:** Both the invite Join and the friend-row Join call `launch_into(QuickPlay::Server(..))` directly. That starts a second game even when one is running, and skips the "Do you want to launch Minecraft to join this server?" question that the server list's Join gets. The server list's path focuses the running game and tells it to connect.
- **Fix:** Route both through `self.join_server(..)`.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 8. "Remove" on a mod deletes the jar permanently with no confirm and no undo
- **Where:** `crates/arctic-app/src/ui/instances/mods_page.rs:348-349`, `crates/arctic-core/src/mods/files.rs:122-127`
- **Now:** The trash icon's tooltip says "Remove". One click calls `mods::remove`, which `remove_file`s both the `.jar` and `.jar.disabled`. No confirmation, no success toast, no trash folder. Instances and worlds using the same icon go to a `.trash` folder; mods do not.
- **Fix:** Move the jar to `mods/.trash/` and show a toast with an Undo action, or keep a two-click confirm like the instance card. At minimum call it "Delete".
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 9. "Delete" on a resource or shader pack is permanent, no confirm
- **Where:** `crates/arctic-app/src/ui/instances/packs.rs:174-175`, `crates/arctic-core/src/mods/packs.rs:127-135`
- **Now:** One click runs `remove_file`/`remove_dir_all` on a pack that may be a 200 MB download. No confirm, no undo, no success toast (only "Couldn't delete it" on failure).
- **Fix:** Same as finding 8: trash folder plus Undo toast, or an inline confirm.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 10. Removing a friend or declining a request is one irreversible click
- **Where:** `crates/arctic-app/src/ui/friends.rs:593-595`, `crates/arctic-app/src/ui/friends.rs:527-528`, `crates/arctic-app/src/ui/friends.rs:705`
- **Now:** The trash icon "Remove friend" immediately sends `FriendAction::Remove`. Decline uses the same action, so a slip on a request is also irreversible. `on_friend_done` then says nothing on success.
- **Fix:** A confirm ("Remove Alice from your friends?") or an Undo toast; at least a success toast.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 11. Deleting a library skin: no confirm, file deleted, no undo
- **Where:** `crates/arctic-app/src/ui/skins/cards.rs:267-270`, `crates/arctic-app/src/ui/skins/mod.rs:676-684`
- **Now:** Right-click "Delete" removes the PNG and the library entry at once, with no toast.
- **Fix:** Confirm, or soft-delete with an Undo toast action.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 12. A chat message is lost when sending fails
- **Where:** `crates/arctic-app/src/ui/chat.rs:272`, `crates/arctic-app/src/ui/chat.rs:402`
- **Now:** The draft is `mem::take`n before the request. On failure the toast says "Message not sent" with the raw error, and the text is gone.
- **Fix:** Keep the draft until the send succeeds (or restore it on error) and add a Retry action.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** no

### 13. Voice room join failures are swallowed and the status says all is well
- **Where:** `crates/arctic-app/src/voice.rs:444`, `crates/arctic-app/src/voice.rs:219`
- **Now:** A failed room check-in (token refused, server down) is `log::info!` only. `status()` then shows "On, nobody else here uses Arctic voice yet". The player hears nobody and is told everything is fine.
- **Fix:** Store the error in the voice state and show "Couldn't join the voice room: …" with Retry.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** no

### 14. Modpack import fails on long pack names and makes duplicate-named instances
- **Where:** `crates/arctic-core/src/mods/modpack.rs:232`, `crates/arctic-core/src/instances/mod.rs:236-241`, `crates/arctic-core/src/instances/mod.rs:265`
- **Now:** `install_file` calls `instances::create` with the pack's own name. Names over 48 characters fail with "Could not install modpack" / "instance names must be 1-48 characters", though the player never typed a name. Installing the same pack twice yields two instances with the same display name because `free_name` (which trims and appends " (2)") is not used.
- **Fix:** Use `free_name(dirs, &index.name)?` in `install_file`.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 15. An unplugged storage drive shows red text with no action, and only in Settings
- **Where:** `crates/arctic-app/src/ui/storage.rs:115-124`
- **Now:** Only Settings → Storage says "{path} isn't available (an unplugged drive?). Arctic uses its usual folder until it's back." There is no "Pick another folder" or "Stop looking for it", and nothing at startup or on Play explains why instances and worlds vanished.
- **Fix:** A startup toast with an "Open Storage settings" action; "Forget that folder" and "Pick another…" buttons in the section.
- **Impact:** high · **Effort:** S · **CHANGES BEHAVIOR:** no

### 16. "Launch failed" is a raw error chain with no action, and never reaches the run's log
- **Where:** `crates/arctic-app/src/runs.rs:384-392`, `crates/arctic-core/src/error.rs:9-23`, `crates/arctic-core/src/net/download.rs:324`, `crates/arctic-core/src/java/mod.rs:254`, `crates/arctic-core/src/launch/mod.rs:285`
- **Now:** Toast "Launch failed" with body `e.chain()`. Typical bodies: "I/O error at C:\Users\…\libraries\…jar: Access is denied", "checksum mismatch for https://piston-data.mojang.com/…", "runtime installed but C:\…\javaw.exe is missing", "Mojang provides no 'java-runtime-delta' Java runtime for linux-i386", "inherited (mod loader) profiles are not supported yet". Raw paths, URLs, internal ids, no Retry or View-logs button. The Logs tab was switched to this run at launch but shows only the "──── Launching … ────" banner, because the error is only `log::error!`ed.
- **Fix:** Push the error into the run's log as an Error line; give the toast "View logs" and Retry actions; map error classes to plain text (download/checksum → "Couldn't download the game files. Check your connection and press Play again."; Io → "Couldn't write to <folder>. Is the disk full or read-only?").
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** no

### 17. An expired Microsoft session becomes a generic launch failure with no "sign in again"
- **Where:** `crates/arctic-app/src/tasks.rs:400-405`, `crates/arctic-core/src/auth/microsoft/oauth.rs:82-84`, `crates/arctic-app/src/ui/accounts.rs:37-131`
- **Now:** The token refresh runs inside the launch with `?`. A revoked or expired refresh token becomes "Launch failed" / "authentication failed: <Microsoft's raw error_description>" (often an AADSTS… sentence), gone after 9 s. The account is not flagged, pressing Play repeats the failure, and the account card offers only Remove. The actual cure (add the account again) is never suggested.
- **Fix:** Catch `Error::Auth` from the refresh step; toast "Sign in to {name} again" with a "Sign in" action that starts the browser flow; show a "Signed out" badge with "Sign in again" on the account card.
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** yes

### 18. Sign-in failures are double-prefixed, raw, and offer no next step
- **Where:** `crates/arctic-app/src/ui/dialogs.rs:50-56`, `crates/arctic-core/src/error.rs:22-23`, `crates/arctic-core/src/auth/microsoft/xbox.rs:103-186`
- **Now:** The dialog shows "Sign-in failed" then `e.to_string()`, which for every auth error reads "authentication failed: …". Known cases name a fix in prose with no button: "…Sign in once at xbox.com to create one.", "This is a child account; an adult must add it to a Microsoft family.", "does not own Minecraft: Java Edition". Developer text reaches players: "Minecraft services rejected this app. New Azure app IDs must be approved by Mojang (see docs/microsoft-auth.md)." Generic failures are raw HTTP codes. The only button is "Try again", which returns to the method chooser.
- **Fix:** Drop the "authentication failed:" prefix; add per-case actions ("Open xbox.com", "Open Microsoft Family", "Get Minecraft", "Use a different account"); turn HTTP codes into "Microsoft's servers didn't answer. Check your connection and try again."; keep the docs pointer in the log only.
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** yes

### 19. Browser sign-in is a silent five-minute wait with no fallback
- **Where:** `crates/arctic-app/src/ui/dialogs.rs:195`, `crates/arctic-app/src/ui/dialogs.rs:227`, `crates/arctic-core/src/auth/microsoft/browser.rs:21`, `crates/arctic-core/src/auth/microsoft/browser.rs:84-94`
- **Now:** The browser flow shows a spinner, "Finish signing in in your browser…" and Cancel. The URL is opened once and never exposed. If the browser opened behind the launcher, in the wrong profile, or not at all, the player waits up to the 5-minute timeout and then gets "authentication failed: timed out waiting for the browser login". The code flow, by contrast, offers "Open sign-in page" and a copy button.
- **Fix:** In the waiting state add "Open the sign-in page again", "Copy link" and "Use a code instead"; after 30 s add "Taking a while?…".
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** no

### 20. A launch cannot be cancelled while it downloads
- **Where:** `crates/arctic-app/src/ui/play.rs:540-579`, `crates/arctic-app/src/ui/play.rs:631-636`, `crates/arctic-app/src/widgets.rs:31`, `crates/arctic-core/src/net/download.rs:166-226`
- **Now:** During Preparing the Play button is in its Progress state and ignores clicks. The Force close tile appears only once the process has spawned. `download_all` has no cancel token. A first Forge launch with the Java runtime and assets on a slow line is minutes of "57%" with no way out except closing the launcher.
- **Fix:** Add a cancel flag to the launch request, checked in the download workers and between stages; turn the Play button into "Cancel" while preparing.
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** yes

### 21. Trashed worlds can never be brought back from the UI
- **Where:** `crates/arctic-app/src/ui/instances/worlds.rs:125-132`, `crates/arctic-core/src/worlds/mod.rs:304-309`
- **Now:** "Move to trash" moves the world to `saves/.trash/<folder>-<unix secs>` and toasts "Moved {name} to trash" with the body "saves/.trash" (a raw relative path). There is no restore function and no UI that lists the trash. The only way back is a file manager plus renaming a timestamped folder by hand.
- **Fix:** Add `worlds::trashed()`/`restore()` and an Undo action on the toast or a "Trash (n)" button on the Worlds page, mirroring `trash.rs` for instances.
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** no

### 22. There is no way to remove a server from the Java server list
- **Where:** `crates/arctic-app/src/ui/servers.rs` (whole file), `crates/arctic-core/src/servers/mod.rs:35-234`
- **Now:** Core has `add`, `move_before` (drag reorder) and `list`, but no remove. A server added by mistake from Discover or Direct join can only be deleted inside Minecraft.
- **Fix:** `servers::remove(game_dir, address)` in core (same NBT rewrite as `move_before`) plus a trash icon or right-click "Remove" on the row with an Undo toast.
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** no

### 23. Migration has no cancel, and the dialog can't be closed while scanning
- **Where:** `crates/arctic-app/src/ui/migrate.rs:131-137`, `crates/arctic-app/src/ui/migrate.rs:484-503`, `crates/arctic-app/src/migrate_tasks.rs:56-58`
- **Now:** The dialog refuses to close while Scanning or Running. Scanning shows only a spinner, yet the scan also measures every world and pack tree of every found instance, which on a big CurseForge or TLauncher install takes tens of seconds. The copy has a progress bar but no Cancel and no cancellation flag, so a wrong multi-gigabyte pick must run to completion.
- **Fix:** Show instances as soon as the scan returns and fill sizes in asynchronously; add Cancel to both stages via an atomic flag checked between files.
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** no

### 24. Moving the storage folder is uncancellable and ends with an unannounced restart
- **Where:** `crates/arctic-app/src/ui/storage.rs:208-223`, `crates/arctic-app/src/ui/storage.rs:163-166`, `crates/arctic-core/src/storage/location.rs:199-235`
- **Now:** "Moving Arctic's files…" shows a byte progress bar and "Arctic restarts when it's done." but has no Cancel and ignores Esc. On success the app calls `restart_after_update` immediately, so the window closes mid-use with no "Done" moment. `move_data` already has a cleanup path for a failed copy that could serve a cancel.
- **Fix:** Add Cancel reusing the existing cleanup; on success show "Moved. Restarting…" for a moment or a "Restart now" button.
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** yes

### 25. Import from another launcher: one locked file aborts the whole instance, with a raw I/O error
- **Where:** `crates/arctic-app/src/migrate_tasks.rs:103-108`, `crates/arctic-core/src/migrate/copy.rs:594-596`, `crates/arctic-core/src/error.rs:9`
- **Now:** A failed outcome is shown as `e.to_string()`, for a copy failure "I/O error at C:\…\mods\foo.jar: Access is denied (os error 5)": a full source path plus an OS error code, with no retry. A single locked file (the other launcher's game still running) aborts the instance and the half-made instance is deleted.
- **Fix:** Skip and note unreadable files ("3 files couldn't be read (is the other launcher running?)") instead of aborting; map Io errors to "Couldn't read <file name>"; offer "Try again" on a failed row.
- **Impact:** high · **Effort:** M · **CHANGES BEHAVIOR:** yes

### 26. Video export on Linux and macOS dead-ends with "install it yourself"
- **Where:** `crates/arctic-core/src/ffmpeg.rs:51-61`, `crates/arctic-app/src/bridge_host.rs:286-291`
- **Now:** `can_download()` is `cfg!(windows)`. Elsewhere the game is told "Install FFmpeg with your package manager, then try again." There is no launcher-side UI for FFmpeg at all: no install button, no status, no way to point at an existing binary, and a failed download reaches the game only as a raw error string.
- **Fix:** Download a static FFmpeg build on Linux and macOS too, or at minimum add a Settings row with "Install FFmpeg" / "Choose ffmpeg…" and a visible status.
- **Impact:** high · **Effort:** L · **CHANGES BEHAVIOR:** yes

## Medium impact

### 27. The tray host and the hidden start read a settings file that no longer exists
- **Where:** `crates/arctic-app/src/main.rs:93`, `crates/arctic-app/src/tray_host.rs:93`, `crates/arctic-core/src/storage/mod.rs:136-137`, `crates/arctic-core/src/profiles.rs:232-252`
- **Now:** Both call `Settings::load` on the unscoped root dirs, whose settings file is `<root>/settings.json`. `migrate_legacy` moves that file into `profiles/<id>/`, so the root file is absent and defaults are used: the tray setting is always treated as on, and `last_version` is always None, so the tray menu's "Play" opens the Play tab instead of launching the last version as the code intends.
- **Fix:** Load the `ProfileStore` and use `profiles.scoped(&dirs)` before reading settings in both places.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 28. "Restore defaults" has no confirmation and wipes more than settings
- **Where:** `crates/arctic-app/src/ui/settings.rs:376-381`, `crates/arctic-core/src/settings.rs:256-307`
- **Now:** One click replaces everything except `last_version` with `Settings::default()`. That resets favorites, muted voice players, the custom backdrop, the crash contact, and bookkeeping flags: `onboarded` and `heard_asked` (setup returns), `tray_asked` (the close question returns), `launcher_version_seen` (the next What's new is skipped). No toast, no undo.
- **Fix:** A confirm listing what resets; preserve bookkeeping flags and personal lists.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 29. The Network section needs Save while the page header promises autosave
- **Where:** `crates/arctic-app/src/ui/settings.rs:47`, `crates/arctic-app/src/ui/network.rs:83-115`
- **Now:** The Settings header says "Changes are saved automatically." but proxy edits sit in a draft until "Save" (disabled until valid). It is the only section that works this way; a player who edits the address and leaves loses it.
- **Fix:** Apply live like the rest, or show "Not saved yet" in the section and exempt it from the header claim.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 30. Turning the tray setting off leaves the icon until restart
- **Where:** `crates/arctic-app/src/app.rs:909-912`
- **Now:** The tray is created when the setting is on and none exists, but nothing ever sets `self.tray = None`. After unticking the checkbox the icon and its Open/Play/Quit menu stay until the launcher restarts.
- **Fix:** Drop the tray when the setting is off, next to where it is created.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 31. The Settings memory slider lacks the guidance onboarding gives
- **Where:** `crates/arctic-app/src/ui/settings.rs:164-181`, `crates/arctic-app/src/ui/onboarding.rs:404-433`
- **Now:** Settings → Memory lets "Maximum" go to 32 GB on any machine, with the hint "4 GB suits vanilla. Giving Java more than it needs can cause stutter." Onboarding shows "Recommended: X GB (this PC has Y GB)", a "Use it" link, caps the slider at RAM minus 1 GB and warns "That's over half your RAM…". None of that appears where people actually tune it.
- **Fix:** Reuse the onboarding logic in the Memory section.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 32. Update status and the manual check live in About, and a failed startup check is invisible
- **Where:** `crates/arctic-app/src/ui/about.rs:155-175`, `crates/arctic-app/src/ui/settings.rs:269-295`, `crates/arctic-app/src/app.rs:1287-1290`, `crates/arctic-app/src/ui/shell.rs:171-180`
- **Now:** Settings → Updates has the channel and four checkboxes but no status and no "Check for updates"; those are in About. A failed startup check sets `Failed { retry: None }`, which the banner hides, so the player never learns the check failed unless they open About.
- **Fix:** Put the status line and "Check for updates" in Settings → Updates; after a failed startup check show a one-line note there.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 33. "Game settings for new instances" is filed under "Data"
- **Where:** `crates/arctic-app/src/ui/settings.rs:35-42`, `crates/arctic-app/src/ui/settings.rs:322-324`
- **Now:** `group_of` maps named sections only; "Game settings for new instances" (FOV, render distance, volumes) falls through to the "Data" chip beside Storage and "Share & back up", while the "Game" chip holds Memory, Versions, Game window and Java.
- **Fix:** Add it to the Game arm.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 34. An unknown crash is explained as "Exit code 1."
- **Where:** `crates/arctic-app/src/runs.rs:553-557`
- **Now:** "{title} closed unexpectedly" / "Exit code {c}." A number nobody can act on, next to a "View logs" link.
- **Fix:** "Minecraft closed on its own. Open the log to see the last thing it did, or send the report." Keep the exit code in the log line.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 35. Snapshots and old versions hide behind a Settings toggle; the picker gives no hint
- **Where:** `crates/arctic-app/src/ui/settings.rs:183-186`, `crates/arctic-app/src/ui/play.rs:426-436`, `crates/arctic-app/src/ui/play.rs:457-463`
- **Now:** The picker has All / Installed / Favorites tabs and a search box. Typing "24w10a" yields "No matching versions". The reason (`show_snapshots` off by default) lives in Settings → Versions, three clicks and a tab switch away.
- **Fix:** A "Snapshots" toggle inside the picker; when the search matches a hidden snapshot, make the empty state say "Snapshots are hidden — show them".
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 36. Quick actions are unlabeled squares, and the two "log" actions behave differently
- **Where:** `crates/arctic-app/src/ui/play.rs:596-637`, `crates/arctic-app/src/ui/play.rs:149-157`, `crates/arctic-app/src/widgets.rs:222`
- **Now:** The tiles are icon-only with tooltips. "Open last game log" hands the file to the OS; "View its log" for other runs opens the Logs tab. "Force close Minecraft" and "Force close this game" are the same action.
- **Fix:** Open the Logs tab from both; unify to "Force close"; consider short labels under the icons.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 37. No guidance when a game hangs before its window appears
- **Where:** `crates/arctic-app/src/ui/play.rs:566-574`, `crates/arctic-app/src/ui/play.rs:674-676`
- **Now:** Starting shows "Starting…" and "{stage}… · {n} s" counting up without end. A broken modpack that never opens a window leaves the player watching "Preparing the game… · 412 s".
- **Fix:** After about two minutes append "Taking long? Force close and check the log" and highlight the Force close tile.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 38. A failed modpack install leaves a never-seen instance in the Trash
- **Where:** `crates/arctic-core/src/mods/modpack.rs:238-241`
- **Now:** On any download or extract error the half-made instance is `instances::remove`d, which moves it to `instances/.trash`. The player sees "Could not install modpack" and then a "Trash (1)" button holding a broken instance they never saw.
- **Fix:** Delete the folder outright on failure.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 39. The create-instance form accepts a name it will then reject
- **Where:** `crates/arctic-app/src/ui/instances/create.rs:134`, `crates/arctic-app/src/ui/instances/create.rs:169-171`, `crates/arctic-app/src/ui/instances/detail.rs:143`
- **Now:** The rename field in instance Settings has `char_limit(48)`, the create form's name field does not. Typing 49+ characters keeps "Create instance" enabled; clicking it closes the dialog and toasts "Could not create instance" / "instance names must be 1-48 characters", losing the whole form.
- **Fix:** Add the limit to the field; keep the dialog open on error.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 40. Cancelling the world-zip picker shows a green "No file chosen" success toast
- **Where:** `crates/arctic-app/src/world_tasks.rs:41`, `crates/arctic-app/src/ui/instances/worlds.rs:283`
- **Now:** Picker cancel returns `Ok("No file chosen")`, which `on_worlds_done` pushes as a Success toast. The modpack picker handles the same case correctly (nothing shown).
- **Fix:** Return a "nothing to say" value and show nothing on cancel.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 41. Partial world-import failures are swallowed
- **Where:** `crates/arctic-app/src/world_tasks.rs:109-123`
- **Now:** Importing three worlds where one fails shows "Imported 2 worlds" (Success) and drops the error. Only when all fail does the player see anything, and then only the last error.
- **Fix:** Report the failures by name in the toast body, as an Error kind when any failed.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 42. "N could not be updated." never says which mods
- **Where:** `crates/arctic-app/src/ui/instances/mod.rs:501-505`, `crates/arctic-app/src/tasks.rs:593`
- **Now:** The update task reduces failures to project ids and the UI to a count: "2 could not be updated." The underlying reasons ("X has no version for Fabric 1.21.4", rate limit) are discarded.
- **Fix:** Carry (title, error) through and list them in the update card.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 43. Resource-pack search is capped at 20 results with no "load more"
- **Where:** `crates/arctic-app/src/ui/instances/packs.rs:301-337`
- **Now:** `run_rp_search` always sends offset 0 with one page, and the results list has no `more_on_scroll`, while mods and modpacks both page. The player sees 20 packs and nothing else.
- **Fix:** Reuse the paging from `browse.rs:142-144`.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 44. Load and search errors are dead-end labels with no Retry, in ten places
- **Where:** `crates/arctic-app/src/ui/instances/browse.rs:113`, `crates/arctic-app/src/ui/instances/packs.rs:329`, `crates/arctic-app/src/ui/instances/modpacks.rs:154`, `crates/arctic-app/src/ui/instances/retarget.rs:139`, `crates/arctic-app/src/ui/instances/loader_switch.rs:103`, `crates/arctic-app/src/ui/servers.rs:94`, `crates/arctic-app/src/ui/discover.rs:200`, `crates/arctic-app/src/ui/friends.rs:100`, `crates/arctic-app/src/ui/skins/gallery.rs:123-125`, `crates/arctic-app/src/ui/skins/panels.rs:139`
- **Now:** "Search failed: network error: timeout: receive response", "Could not load the versions: …", "Couldn't check the mods: …", "Your server list couldn't be read: {e}", "Friends couldn't be loaded: {e}", "Arctic looks are unavailable: {e}" are plain labels. Only the Mods page and the Bedrock version list have a Retry button. The text comes from the core error's Display ("network error: {0}").
- **Fix:** One shared error row with a Retry button and plain wording ("Couldn't reach Modrinth").
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 45. "Play this world (1.20+)" is offered on every version and silently does nothing before 1.20
- **Where:** `crates/arctic-app/src/ui/instances/worlds.rs:152`, `crates/arctic-core/src/launch/mod.rs:946`
- **Now:** The tooltip carries the caveat, but the button is shown for a 1.16 instance too. Clicking starts the game at the title screen; the only feedback is a log line "this version can't open a world directly".
- **Fix:** Hide or disable the button when the version has no `quickPlaySingleplayer` placeholder, or toast why.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 46. The migration "done" screen gives no way to open or play what came over
- **Where:** `crates/arctic-app/src/ui/migrate.rs:507-527`, `crates/arctic-app/src/ui/migrate.rs:289-306`
- **Now:** After "Everything came over" the list has only a Done button; rows are plain labels "{launcher} · {name} → {instance}". The toast body "See the details in the window." adds nothing while the window is open.
- **Fix:** Make successful rows open the instance; add "Open" / "Play" for a single import.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 47. The replay list goes stale after a game session
- **Where:** `crates/arctic-app/src/ui/instances/replays.rs:28-34`, `crates/arctic-app/src/ui/instances/replays.rs:43-45`, `crates/arctic-app/src/ui/instances/replays.rs:228-231`
- **Now:** The cached list is refreshed only when the instance changes, on trash, or via the manual Refresh button. Record a replay, quit, return to the page: it isn't there. The Screenshots tab, by contrast, rescans every 3 s on its own.
- **Fix:** Clear the cache when a run for that instance ends, or rescan on a timer; then drop the Refresh button.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 48. A thumbnail that failed to load looks exactly like one still loading
- **Where:** `crates/arctic-app/src/ui/screenshots.rs:189-199`, `crates/arctic-app/src/ui/screenshots.rs:461-471`
- **Now:** `Thumb::Failed` (error discarded) falls into the same arm as Loading and paints the same grey rectangle. A corrupt PNG is a permanent blank tile with no hint and no retry.
- **Fix:** Paint a broken-image glyph with a "Couldn't open" tooltip; still open the OS viewer on click.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 49. Trash actions have no Undo and show internal folder paths
- **Where:** `crates/arctic-app/src/ui/instances/worlds.rs:131`, `crates/arctic-app/src/ui/instances/detail.rs:207`, `crates/arctic-app/src/ui/instances/detail.rs:621`, `crates/arctic-app/src/ui/instances/replays.rs:198-204`, `crates/arctic-app/src/ui/screenshots.rs:298`, `crates/arctic-app/src/ui/bedrock.rs:694-698`, `crates/arctic-app/src/ui/bedrock.rs:840-844`, `crates/arctic-app/src/ui/profiles.rs:270`
- **Now:** Toast bodies and dialog text say "saves/.trash", "Moved to instances/.trash", "replay_recordings/.trash", "com.mojang\\arctic-trash", "profiles/.trash". Screenshot and video trashing show no toast at all. None offer Undo; only instances have a Trash list to restore from.
- **Fix:** One toast pattern: "Moved to trash" with an Undo action (rename back) and no path; "Open trash" where a list is not worth building.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 50. Moving an instance's game folder shows a fake progress bar and throws the real progress away
- **Where:** `crates/arctic-app/src/ui/storage.rs:422-426`, `crates/arctic-app/src/storage_tasks.rs:108-113`, `crates/arctic-core/src/instances/mod.rs:305-311`
- **Now:** `set_game_folder` takes a progress callback but the task passes `&|_, _| {}`; the dialog shows an indeterminate animated bar ("Moving the instance's files…") for what may be a 10 GB worlds folder, and refuses Esc while busy. The launcher-wide move already reports bytes.
- **Fix:** Wire the callback through an event like the launcher-wide move; add Cancel.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 51. One feature, three names: "Move to Arctic", "From another launcher", "Bring over"
- **Where:** `crates/arctic-app/src/ui/migrate.rs:125`, `crates/arctic-app/src/ui/migrate.rs:226`, `crates/arctic-app/src/ui/settings.rs:344`, `crates/arctic-app/src/ui/instances/mod.rs:286-287`, `crates/arctic-app/src/ui/onboarding.rs:644-646`
- **Now:** The button says "From another launcher" (filed under "Share & back up"), the dialog is titled "Move to Arctic" though it copies (its own text says "The originals stay as they are"), the action is "Bring over 3", the toast says "came over".
- **Fix:** One name ("Import from another launcher", action "Import 3"); move the button out of "Share & back up".
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 52. "Minecraft skin change failed" fires on plain loading, and the fix it names has no button
- **Where:** `crates/arctic-app/src/ui/skins/mod.rs:794-797`, `crates/arctic-app/src/ui/skins/mod.rs:315`, `crates/arctic-core/src/skins/api.rs:287`
- **Now:** Every `SkinAccount` error toasts "Minecraft skin change failed", including the initial read when nothing was changed. The 401 text is "Your sign-in expired. Sign in to this account again." with no Sign in action and no Retry anywhere on the tab.
- **Fix:** Title by what was attempted ("Couldn't load your Minecraft skin"); add an action that opens the sign-in; add Retry.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 53. Ping failures print the raw transport error under each server
- **Where:** `crates/arctic-app/src/ui/servers.rs:496`, `crates/arctic-app/src/ui/servers.rs:355`, `crates/arctic-app/src/ui/bedrock.rs:1065`, `crates/arctic-core/src/servers/mod.rs:219`
- **Now:** The reason label is the ping crate's error text unchanged; direct-join shows "No answer: {e}".
- **Fix:** Map to "Offline", "No such address", "Timed out"; raw text on hover.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 54. "…are loading." forever when the cosmetics catalog failed, and clicks silently do nothing
- **Where:** `crates/arctic-app/src/ui/skins/cards.rs:434`, `crates/arctic-app/src/ui/skins/cards.rs:667`, `crates/arctic-app/src/ui/skins/mod.rs:333-340`, `crates/arctic-app/src/ui/skins/cards.rs:184`
- **Now:** "Arctic capes are loading." and "Cosmetics are loading." are shown whenever the catalog state is None, which includes the error case. When the account's look failed to load, every card click is gated by `can_wear` and dropped with no feedback.
- **Fix:** Distinguish loading from failed and show the error with Retry; disable cards or toast why when wearing is not possible.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 55. Changing the real Mojang skin or cape has no confirmation
- **Where:** `crates/arctic-app/src/ui/skins/cards.rs:256-257`, `crates/arctic-app/src/ui/skins/panels.rs:473-474`, `crates/arctic-app/src/ui/skins/cards.rs:587-596`, `crates/arctic-core/src/skins/api.rs:290`
- **Now:** "Set as Minecraft skin" (hover: "Changes your real Minecraft skin, seen by everyone"), "Reset Minecraft skin" and the Minecraft cape picker act on one click; the hover text is the only warning. Mojang rate-limits changes ("Too many changes at once").
- **Fix:** A short confirm for the Mojang-side actions; the Arctic look stays instant.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 56. Naming drift: three kinds of "code", two kinds of "Share", tab labels that don't match page titles
- **Where:** `crates/arctic-app/src/ui/friends.rs:167`, `crates/arctic-app/src/ui/together.rs:125`, `crates/arctic-app/src/ui/sharing.rs:476`, `crates/arctic-app/src/ui/sharing.rs:283-285`, `crates/arctic-app/src/ui/settings.rs:338`, `crates/arctic-app/src/ui/together.rs:166`, `crates/arctic-app/src/ui/nav.rs:27-28`, `crates/arctic-app/src/ui/together.rs:79`
- **Now:** "Friend code", "invite code" and "share code" are three different things. "Share your profile" and "Export profile" describe bundles while "Start sharing" / "Stop sharing" means hosting a world. The sidebar tab is "Friends" but its page is titled "Play together"; the sidebar says "Cosmetics" while the code, tips and docs say Skins.
- **Fix:** Hosting becomes "Host" / "Stop hosting" with a "world code"; keep "share code" for bundles; one word for Export/Share; make tab label and page title agree.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 57. The host card tells you to copy-paste the invite code though one-click Invite exists
- **Where:** `crates/arctic-app/src/ui/together.rs:183`, `crates/arctic-app/src/ui/together.rs:129`, `crates/arctic-app/src/ui/friends.rs:611`
- **Now:** Step 3 reads "Send your friends the invite code." next to a Copy button. The friend row's "Invite" button already sends the code as an invite the friend accepts with one click.
- **Fix:** Step 3 becomes "Click Invite next to a friend, or copy the code."; list online friends with Invite buttons inside the host card.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 58. Generic toast titles with raw bodies: "Friends", "Gallery", "Bedrock"
- **Where:** `crates/arctic-app/src/ui/friends.rs:707`, `crates/arctic-app/src/ui/skins/gallery.rs:384`, `crates/arctic-app/src/ui/bedrock.rs:1196`, `crates/arctic-core/src/cosmetics.rs:279`
- **Now:** The title is the feature name and the body is the server's or OS's error text ("Arctic: {detail}", "The Arctic server returned {status}.").
- **Fix:** Title says what failed ("Couldn't send the friend request"); carry the attempted action into the result event.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 59. The Play together error is raw, sticky and cannot be dismissed
- **Where:** `crates/arctic-app/src/ui/together.rs:89-90`, `crates/arctic-app/src/ui/together.rs:305`, `crates/arctic-app/src/ui/together.rs:328`, `crates/arctic-app/src/ui/together.rs:400`
- **Now:** "Could not start networking: {e}", "Could not create your invite key: {e}" and code-parse errors render in red with no close button and stay until the next action succeeds.
- **Fix:** Dismiss button, plain-language mapping, Retry.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 60. The proxy warning is an Error toast on every single launch
- **Where:** `crates/arctic-app/src/runs.rs:630-662`
- **Now:** With a proxy on, every start of a Forge or no-client instance pushes an Error toast "The proxy won't cover this game's servers". It is a notice, not an error, and it repeats forever.
- **Fix:** Info kind, shown once per instance, with an action "Turn on Arctic Client" where supported.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 61. Four user-facing strings contain runs of 15-22 literal spaces
- **Where:** `crates/arctic-app/src/app.rs:839`, `crates/arctic-app/src/ui/settings.rs:291`, `crates/arctic-app/src/voice.rs:213`, `crates/arctic-app/src/ui/onboarding.rs:459`
- **Now:** String literals were wrapped in the source without `\` continuations, so the UI renders "…(Instances → New).                      Or close the…", "…(or from                  the banner's Restart now)…", "Check Windows Settings →                  Privacy → Microphone…", "…HUD in game.                      Turn it back on…".
- **Fix:** Add the `\` continuation or join the pieces.
- **Impact:** med · **Effort:** S · **CHANGES BEHAVIOR:** no

### 62. Switching account leaves the Play tab
- **Where:** `crates/arctic-app/src/ui/play.rs:331-338`, `crates/arctic-app/src/ui/play.rs:254-291`
- **Now:** The account chip's tooltip is "Switch account", but clicking it jumps to the Accounts tab. The instance name right beside it opens an in-place popup.
- **Fix:** The same popup pattern: accounts with faces and "Add account…" at the bottom.
- **Impact:** med · **Effort:** M · **CHANGES BEHAVIOR:** no

### 63. Join while the game is starting tells the player to come back and click again
- **Where:** `crates/arctic-app/src/app.rs:649-655`
- **Now:** "Minecraft is still starting" / "Try Join again when its window is up."
- **Fix:** Queue the address on the run and send it on the window-ready event ("Joining {server} once the game is up").
- **Impact:** med · **Effort:** M · **CHANGES BEHAVIOR:** yes

### 64. Clash fixes expose raw mod ids and may install the wrong Modrinth project
- **Where:** `crates/arctic-core/src/migrate/clashes.rs:353`, `crates/arctic-core/src/migrate/clashes.rs:376`, `crates/arctic-core/src/mods/modrinth.rs:288`
- **Now:** `project_of` knows three well-known ids; everything else is shown as-is: "Sodium Extra needs cloth_config_lite, which isn't installed" with the button "Get cloth_config_lite". Clicking installs that id as a Modrinth slug; when it doesn't match, the toast is "Could not install mod" / "cloth_config_lite was not found on Modrinth".
- **Fix:** Resolve mod id to project via Modrinth search or the hash lookup in `identify.rs` before offering "Get"; fall back to "Search Modrinth for …".
- **Impact:** med · **Effort:** M · **CHANGES BEHAVIOR:** no

### 65. Hand-added mods show as file names forever though recognition exists
- **Where:** `crates/arctic-app/src/ui/instances/mods_page.rs:329-339`, `crates/arctic-core/src/mods/identify.rs:35`
- **Now:** A dropped jar lists as "sodium-fabric-0.6.5+mc1.21.4.jar" with a placeholder icon. `identify::recognize` (SHA-1 lookup to title, icon and version) runs only during retarget, migration and sharing, never on the Mods page, so "Look for updates" also skips these mods.
- **Fix:** Run recognition in the background when the scan finds untracked jars.
- **Impact:** med · **Effort:** M · **CHANGES BEHAVIOR:** no

### 66. Installed packs are listed by raw file name and search never marks "Installed"
- **Where:** `crates/arctic-app/src/ui/instances/packs.rs:171`, `crates/arctic-app/src/ui/instances/packs.rs:400`, `crates/arctic-app/src/ui/instances/packs.rs:361-373`
- **Now:** Rows show "Faithful 32x - 1.21.zip"; the toast is "Installed Faithful 32x - 1.21.zip". Unlike mods, search results never show an Installed marker, so a second click downloads a duplicate.
- **Fix:** Track installed packs like `mods.json`, show title and icon, mark results installed.
- **Impact:** med · **Effort:** M · **CHANGES BEHAVIOR:** no

### 67. The result of a version change or loader switch is crammed into a 4.5-second toast
- **Where:** `crates/arctic-app/src/ui/instances/retarget.rs:257-265`, `crates/arctic-app/src/ui/instances/loader_switch.rs:351-359`
- **Now:** The only report is a Success toast "Now on 1.21.4" with the body "7 mods updated, turned off: Sodium Extra, Iris, …, couldn't download: xyz, abc". The lists the player must act on vanish in 4.5 s and the preview card is cleared.
- **Fix:** Keep a results card until dismissed, with "Turned off" rows and Retry for failed downloads.
- **Impact:** med · **Effort:** M · **CHANGES BEHAVIOR:** no

### 68. Importing a share code or profile has no Cancel and locks the dialog
- **Where:** `crates/arctic-app/src/ui/sharing.rs:460-467`, `crates/arctic-core/src/sharing/instance.rs:13`
- **Now:** The Working stage shows progress only and the dialog refuses to close while busy. An instance with up to 1000 mods downloads with no way out.
- **Fix:** A cancel flag passed to the import (same pattern as the Bedrock install) and a Cancel button; the install already removes the half-made instance on error.
- **Impact:** med · **Effort:** M · **CHANGES BEHAVIOR:** no

### 69. Long instance tasks have no Cancel and some have no progress
- **Where:** `crates/arctic-app/src/ui/instances/modpacks.rs:116-141`, `crates/arctic-app/src/ui/instances/browse.rs:149-193`, `crates/arctic-app/src/ui/instances/retarget.rs:188-200`, `crates/arctic-app/src/ui/instances/detail.rs:243-247`, `crates/arctic-app/src/ui/instances/worlds.rs:42-44`
- **Now:** A gigabyte modpack shows a bar and "Installing modpack…" but no way to stop. Duplicate shows only a spinner "Copying…" (no progress callback in core). World backup and import show a bare spinner. Only login takes a cancel flag.
- **Fix:** Thread a cancel flag through `download_all` and the copy loops; add Cancel buttons; add byte progress to duplicate and backup.
- **Impact:** med · **Effort:** L · **CHANGES BEHAVIOR:** no

## Low impact

### 70. "Later" on the update banner lasts one session only
- **Where:** `crates/arctic-app/src/ui/shell.rs:204-209`, `crates/arctic-app/src/app.rs:1281-1284`
- **Now:** `dismissed: true` is in memory only; "Arctic Launcher X is available." returns on every start for players with auto-install off, with no "Skip this version".
- **Fix:** Persist a skipped version in settings and offer "Skip this version".
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 71. Restart and update failures show raw I/O text and internal file names
- **Where:** `crates/arctic-app/src/ui/shell.rs:238`, `crates/arctic-app/src/ui/shell.rs:252-253`, `crates/arctic-core/src/update/mod.rs:219-224`, `crates/arctic-core/src/update/mod.rs:265-272`
- **Now:** "Could not restart" with the body "I/O error at C:\…\arctic-launcher.exe: …. Please reopen the launcher."; the banner says "Update failed: checksum mismatch for arctic-launcher-windows-x64.exe" or "… could not install update: Access is denied. (os error 5)".
- **Fix:** Plain variants ("The downloaded file was damaged; trying again usually fixes it", "Windows wouldn't let Arctic replace itself; close antivirus prompts or run once as administrator"); details to the log.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 72. Button vocabulary is inconsistent across dialogs
- **Where:** `crates/arctic-app/src/ui/dialogs.rs:327-333`, `crates/arctic-app/src/app.rs:700-728`, `crates/arctic-app/src/ui/nav.rs:234-237`, `crates/arctic-app/src/ui/about.rs:155`, `crates/arctic-app/src/ui/shell.rs:226`
- **Now:** The game-switch dialog uses "No" / "Yes, always" / "Yes, this time" and the join dialog "Cancel" / "Yes" ("Do you want to launch Minecraft to join this server?" while the main button says PLAY), where every other dialog uses verbs ("Keep in tray" / "Quit Arctic"). The nav menu has "Add account…" with an ellipsis next to "Manage accounts" without. About says "Update installed; restart to finish." while the banner says "Update installed: it's used next time you open the launcher."
- **Fix:** Verb buttons everywhere ("Don't switch" / "Switch once" / "Always allow"; "Start Minecraft and join {server}?" with "Cancel" / "Play"); one ellipsis rule; one installed-update sentence.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 73. The Accounts tab's empty state talks about accounts that don't exist
- **Where:** `crates/arctic-app/src/ui/accounts.rs:16-21`
- **Now:** With zero accounts the header still reads "Click an account to play as it. Sign-ins stay on this PC, encrypted for your user." above a lone dashed "Add account" card.
- **Fix:** When empty, say "Add the Microsoft account that owns Minecraft to start playing." and make the card the focus.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 74. Tray "Play" and `--launch` dead-end without an account or with a bad version id
- **Where:** `crates/arctic-app/src/app.rs:940-945`, `crates/arctic-app/src/app.rs:803-805`, `crates/arctic-app/src/app.rs:1071-1076`
- **Now:** Tray "Play" calls `launch_selected`, which returns silently when there is no active account. `--launch` without an account shows "Choose an account to play" with no action; with an unknown id it shows "Unknown version '1.21.x'" with an empty body.
- **Fix:** Open the Add account dialog and the Accounts tab in the first two cases; "Pick one from the version list." and open the picker in the third.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 75. "Data folder" in About is a plain string
- **Where:** `crates/arctic-app/src/ui/about.rs:105`
- **Now:** "Data folder: C:\Users\…\ArcticLauncher" is a muted label, not clickable or copyable, while Manage profiles has an "Open folder" button.
- **Fix:** An "Open data folder" button with the path on hover.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 76. Profile removal exposes an internal path and offers no restore
- **Where:** `crates/arctic-app/src/ui/profiles.rs:270`
- **Now:** "Remove this profile? Its folder (worlds included) is moved to profiles/.trash, not deleted." There is no UI to see or restore trashed profiles.
- **Fix:** "…is kept in the Trash and can be restored" with an "Open folder" button, or a Trash list.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 77. Disabled Microsoft tiles give no pointer, and a dropped-in client id needs a restart
- **Where:** `crates/arctic-app/src/ui/dialogs.rs:68-72`, `crates/arctic-app/src/app.rs:323`, `crates/arctic-core/src/auth/microsoft/config.rs:27-52`
- **Now:** Without a client id both Microsoft tiles say "Microsoft sign-in isn't available in this build" with no hint where to get one (the hint is a debug log). `msa_configured` is computed once at start, so adding `msa.json` requires a restart.
- **Fix:** A "Learn more" link to docs/building.md on the tile; re-check the config when the dialog opens.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 78. "Try again" after an expired device code goes back to the method chooser
- **Where:** `crates/arctic-app/src/ui/dialogs.rs:53-55`, `crates/arctic-core/src/auth/microsoft/oauth.rs:107`
- **Now:** After "authentication failed: the login code expired, please try again", the single button returns to the chooser, so the player re-picks the method before getting a new code. The 15-minute expiry is never shown while waiting.
- **Fix:** Remember the last method and restart it directly; show "Code expires in N min".
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 79. Messages point to a place in the UI instead of offering a button
- **Where:** `crates/arctic-app/src/app.rs:836-842`, `crates/arctic-app/src/ui/instances/detail.rs:353`, `crates/arctic-app/src/ui/instances/detail.rs:384`
- **Now:** "…make a new instance for it (Instances → New). Or close the {} game first." (the button is actually called "New instance"); "Settings → Arctic Client → Play with Arctic Client. Turn it on there for this instance's switch below to take effect."; "…make a Vanilla or Fabric instance of the same version to play with it." All plain text.
- **Fix:** "New instance for {version}" action on the toast; "Turn on" and "Make a Fabric instance" buttons in the detail page.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 80. "last today" / "last yesterday"
- **Where:** `crates/arctic-app/src/ui/play.rs:95-99`, `crates/arctic-app/src/ui/stats.rs:91`, `crates/arctic-core/src/playtime.rs:226-227`
- **Now:** "Played 3 h · 4 sessions · last today".
- **Fix:** "played today" / "last played yesterday" / "last played 3 days ago".
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 81. Two "ago" dialects on the same tab
- **Where:** `crates/arctic-app/src/ui/play_worlds.rs:111-115`, `crates/arctic-core/src/playtime.rs:224-228`
- **Now:** World tiles say "Just now", "5 min ago", "3 h ago", "Yesterday"; the play-time line says "today", "yesterday", "3 days ago". Different capitalization and granularity side by side.
- **Fix:** One `ago` helper in core, sentence case.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 82. The version picker's Installed tab says "No matching versions" when nothing is installed
- **Where:** `crates/arctic-app/src/ui/play.rs:457-463`
- **Now:** The Installed tab with nothing downloaded shows "No matching versions". Favorites already has its own text.
- **Fix:** "Nothing downloaded yet. Pick a version and press Play."
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 83. Version dates are raw ISO
- **Where:** `crates/arctic-app/src/ui/play.rs:397-401`
- **Now:** `release_time.chars().take(10)` gives "2024-12-03 · snapshot".
- **Fix:** A short readable date ("3 Dec 2024") or relative ("2 years ago").
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 84. Profile-load errors at startup are raw I/O text with no action
- **Where:** `crates/arctic-app/src/session.rs:327-354`
- **Now:** "Could not create data folders" / "I/O error at C:\…: Access is denied"; "Settings were unreadable, using defaults" with the serde error. No "Choose another folder" or "Open folder".
- **Fix:** Plain body and an action to Settings → Storage.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 85. Modpack version rows show raw Modrinth ids
- **Where:** `crates/arctic-app/src/ui/instances/modpacks.rs:265-270`
- **Now:** "1.21.4 · fabric · release", "1.20.1 · neoforge, forge · beta": loader facet ids and a lowercase API enum.
- **Fix:** Map through the loader label and capitalize the type.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 86. World backup opens a file manager unasked and names the zip in the toast
- **Where:** `crates/arctic-app/src/world_tasks.rs:151-157`
- **Now:** The toast is "Backed up {name} to World-1731000000.zip" and the backups folder pops open in the OS every time.
- **Fix:** "Backed up {name}" with an "Open folder" action instead of opening it.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 87. Mod update checks are manual only
- **Where:** `crates/arctic-app/src/ui/instances/mods_page.rs:60-69`
- **Now:** Updates are found only when the player clicks "Look for updates"; nothing checks on open. The button disappears while busy (not disabled), shifting the layout.
- **Fix:** Check once per session on first open and show a count badge; keep the button in place but disabled while checking.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 88. The Java path is a text box with a Windows-only hint and no Browse
- **Where:** `crates/arctic-app/src/ui/instances/detail.rs:293-306`
- **Now:** The hint "C:\Program Files\Java\bin\javaw.exe" shows on Linux and macOS too; the player must type or paste a path. "That file doesn't exist; the managed Java will be used." offers no picker.
- **Fix:** A "Browse…" button and a platform-specific hint.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 89. "Remove", "Delete" and "Move to trash" are used interchangeably
- **Where:** `crates/arctic-app/src/ui/instances/detail.rs:210`, `crates/arctic-app/src/ui/instances/mods_page.rs:348`, `crates/arctic-app/src/ui/instances/packs.rs:174`, `crates/arctic-app/src/ui/instances/worlds.rs:125`, `crates/arctic-app/src/ui/instances/trash.rs:78`, `crates/arctic-app/src/ui/screenshots.rs:298`, `crates/arctic-app/src/ui/instances/replays.rs:118`
- **Now:** "Remove" on an instance is recoverable; "Remove" on a mod is not. "Delete" on a pack is permanent while "Delete for good" is used elsewhere to mark permanence. Screenshots say "Delete (moves it to .trash)", replays say "Move to trash".
- **Fix:** "Move to trash" for recoverable, "Delete" for permanent, everywhere.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 90. World rows lead with the on-disk folder name
- **Where:** `crates/arctic-app/src/ui/instances/worlds.rs:116-121`
- **Now:** The detail line is "{folder} · {ago} · {size}", e.g. "New World (3) · 2 days ago · 48 MB", where the folder usually duplicates or confusingly differs from the displayed name.
- **Fix:** Drop the folder from the row; show it on hover.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 91. The loader build picker shows raw build ids and needs a manual pick to update
- **Where:** `crates/arctic-app/src/ui/instances/loader_switch.rs:112-144`, `crates/arctic-app/src/ui/instances/create.rs:241-250`
- **Now:** A combo of ids like "0.16.9  (stable)" and "21.4.1-beta"; the player must scroll and choose "Use this build". The create form auto-selects the newest stable, but the Settings card offers no one-click "Update to newest stable".
- **Fix:** "A newer stable build is available (0.16.10) – Update" when one exists; keep the combo as an advanced fallback.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 92. "0.0 GB to copy", full source paths and a "Folder:" prefix in the migration chooser
- **Where:** `crates/arctic-app/src/ui/migrate.rs:216`, `crates/arctic-core/src/migrate/sources/managers.rs:253-254`, `crates/arctic-core/src/migrate/sources/mod.rs:91-94`
- **Now:** "0.0 GB to copy" for nothing selected or anything under 50 MB. Scan notes read "Prism Launcher: No instances in C:\Users\x\AppData\Roaming\PrismLauncher\instances" and "Folder: No instances there that Arctic can read".
- **Fix:** Use the adaptive `format_bytes`, hide at zero, drop the path, render Folder notes without the prefix.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 93. Picking a non-empty storage folder asks for two confirmations
- **Where:** `crates/arctic-app/src/ui/storage.rs:292-300`, `crates/arctic-app/src/ui/storage.rs:335-341`
- **Now:** "This folder has other things in it … Arctic can keep its files in a folder of its own inside it" with "Use that folder", which only re-runs the check on the subfolder and opens a second dialog ("Move Arctic's files here?") to confirm again.
- **Fix:** Go straight to the move, or label the first button "Next".
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 94. Unfinished replay recordings are deleted silently at every start
- **Where:** `crates/arctic-app/src/app.rs:270-278`, `crates/arctic-core/src/replays.rs:60-78`
- **Now:** A thread runs `clear_all_temp` over every instance's replay temp folder and removes everything, logging only. After a crash this is the player's last recording and it's gone with no notice or recover option.
- **Fix:** Move leftovers to the replay trash, or a one-time "A recording from a crashed game was found — Keep / Discard" toast.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 95. "Couldn't" and "Could not" are both used, about evenly
- **Where:** `crates/arctic-app/src` (37 strings with "Couldn't", 53 with "Could not"; e.g. `crates/arctic-app/src/ui/storage.rs:224-236`, `crates/arctic-app/src/ui/screenshots.rs:308-312`, `crates/arctic-app/src/ui/instances/replays.rs:121-125`)
- **Now:** Neighbouring toasts read "Couldn't delete it" and "Could not move the video". The bodies are also `e.to_string()` with full paths and OS error codes.
- **Fix:** One form (the README voice is contractions); one helper that maps Io errors to "Couldn't write to <file name> (<short reason>)".
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 96. No way to save a log or open a crash report
- **Where:** `crates/arctic-app/src/ui/logs.rs:257-277`, `crates/arctic-app/src/runs.rs:577-597`
- **Now:** The Logs toolbar offers "Open log file", "Copy shown lines" (filtered lines only) and "Send this log to the Arctic staff". There is no "Save as…", and a crash report can only be sent to Arctic; the player can't open the `crash-reports` folder or export it for a mod's issue tracker.
- **Fix:** "Save log…" (full text) and an "Open crash report" action on the crash toast and Logs tab.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 97. Screenshots: redundant Refresh, misleading filtered-empty state, no folder access when empty
- **Where:** `crates/arctic-app/src/ui/screenshots.rs:76-83`, `crates/arctic-app/src/ui/screenshots.rs:91-95`, `crates/arctic-app/src/ui/screenshots.rs:115-120`, `crates/arctic-app/src/ui/screenshots.rs:315-319`
- **Now:** The tab rescans every 3 s yet still shows a Refresh button. "No screenshots yet. Press F2 in game to take one." is also shown when the instance filter hides everything. "Show in folder" exists only inside the preview, so an empty tab has no way to open the folder.
- **Fix:** Remove Refresh; "No screenshots from {instance} yet" when filtered; "Open folder" in the header.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 98. Migration excludes screenshots by default without saying so
- **Where:** `crates/arctic-app/src/ui/migrate.rs:74-86`
- **Now:** Every category with items is ticked except Screenshots (a code comment says "often large; opt in"). The box sits unticked among ticked ones with no explanation.
- **Fix:** Keep the default but add a hint ("off by default: 1.2 GB").
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 99. Own capes are named "Your cape 3" and renumber themselves
- **Where:** `crates/arctic-app/src/ui/skins/cards.rs:480`, `crates/arctic-app/src/skin_tasks.rs:300`
- **Now:** Names are `format!("Your cape {}", own.len() - i)`, so they shift whenever one is added or removed. There is no rename (library skins have one). The chosen file's name is discarded on upload.
- **Fix:** Keep the file stem as the name and allow Rename.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 100. Voice "couldn't start" is raw, retries silently, and the status is buried
- **Where:** `crates/arctic-app/src/voice.rs:270`, `crates/arctic-app/src/ui/settings.rs:299`, `crates/arctic-app/src/ui/voice_settings.rs:121`
- **Now:** "Voice chat couldn't start: {e}" (an engine or device error) is retried every 15 s with no user action. Status appears only under Settings → Voice chat. "player(s)" is used instead of a plural.
- **Fix:** Plural handling; a "Retry now" button; voice status on the Play tab while in game.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 101. Discover: "Add an account first." has no action, and the Premium filter is dead code
- **Where:** `crates/arctic-app/src/ui/discover.rs:461`, `crates/arctic-app/src/ui/discover.rs:146-153`, `crates/arctic-core/src/servers/public.rs:49-62`
- **Now:** The message has no button. The filter row offers All / Cracked / Partners while an `Access::Premium` arm exists unused, so there is no "Microsoft accounts only" filter.
- **Fix:** Link to Accounts; add the Premium chip or delete the dead arm.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 102. Bedrock world rename drops the edit when focus leaves the field
- **Where:** `crates/arctic-app/src/ui/bedrock.rs:664-674`
- **Now:** Losing focus clears the rename state and the rename only happens if Enter was pressed in the same frame; clicking elsewhere silently discards. Library skins get Save and Cancel buttons.
- **Fix:** Save on blur, or show Save/Cancel like the skin rename.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** yes

### 103. The friend code is shown twice on the same page
- **Where:** `crates/arctic-app/src/ui/friends.rs:184-189`, `crates/arctic-app/src/ui/friends.rs:201-210`
- **Now:** "Your code" with a copy button, and a few rows later "code {}" with another copy button.
- **Fix:** Keep one.
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 104. Keeping an uploaded cape can fail silently
- **Where:** `crates/arctic-app/src/ui/skins/mod.rs:193`
- **Now:** The failure is `log::warn!` only. The cape is worn but never appears under "Your own…" for reuse, and nothing tells the player.
- **Fix:** Toast "Couldn't keep this cape for later: …".
- **Impact:** low · **Effort:** S · **CHANGES BEHAVIOR:** no

### 105. "Play another copy" makes the player switch accounts by hand first
- **Where:** `crates/arctic-app/src/ui/play.rs:596-609`
- **Now:** The tooltip reads "Play another copy (switch account first for a second player)". Launching with the same account twice is the likely mistake.
- **Fix:** Clicking opens an account chooser (excluding accounts already in a running copy) and launches with the pick.
- **Impact:** low · **Effort:** M · **CHANGES BEHAVIOR:** no
