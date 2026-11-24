//#if MC >= 1.13
package com.arcticlauncher.mod;

import com.arcticlauncher.client.replay.ReplayBackend;
import com.arcticlauncher.client.replay.ReplayMenu;
import com.arcticlauncher.client.replay.ReplayViewer;
import com.arcticlauncher.client.replay.Replays;
import java.io.File;
import java.nio.ByteBuffer;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.BooleanSupplier;
import net.minecraft.client.Minecraft;

/**
 * Development check, only with {@code -Darctic.selftest=replay}: records a
 * few seconds in a test world (moving, swinging, with mobs and block
 * changes), keeps it, leaves, watches it, jumps forward and back, tries
 * each camera and grabs a frame the way video export does, then quits.
 * Every step logs "replaytest: …"; a failure logs "replaytest: FAILED".
 */
public final class ReplayTest {
	// Disconnect clears Minecraft's executor queue. A poll queued behind the
	// export's stopWatching callback would be discarded and executeBlocking
	// would wait forever even though the export succeeded. Frames keep running
	// across world changes and while a replay is paused.
	private static final java.util.Queue<Runnable> QUEUED = new java.util.concurrent.ConcurrentLinkedQueue<Runnable>();
	private static boolean inFrame;
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-replaytest");
		t.setDaemon(true);
		return t;
	});
	private static final int WAIT_LIMIT_S = Integer.getInteger("arctic.waitlimit", 120);
	private static final AtomicReference<File> manual = new AtomicReference<File>();
	private static long videoSince;

	private ReplayTest() {}

	static boolean requested() {
		String mode = System.getProperty("arctic.selftest");
		return "replay".equals(mode) || "replayopen".equals(mode) || "replayperf".equals(mode) || "replayperfopen".equals(mode) || "replaymid".equals(mode) || "replayaudit".equals(mode);
	}

	static void start() {
		log("scheduled");
		if ("replayaudit".equals(System.getProperty("arctic.selftest"))) {
			auditStart();
			return;
		}
		if ("replayperf".equals(System.getProperty("arctic.selftest"))) {
			TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(ReplayTest::perfSession)), 10, TimeUnit.SECONDS);
			return;
		}
		if ("replayperfopen".equals(System.getProperty("arctic.selftest"))) {
			File file = new File(System.getProperty("arctic.replayfile", ""));
			TIMER.schedule(() -> run(() -> perfOpen(file)), 10, TimeUnit.SECONDS);
			return;
		}
		if ("replayopen".equals(System.getProperty("arctic.selftest"))) {
			// Launched with a replay to open (arctic launch --replay): just check it opens.
			when("opened from the launcher", () -> Replays.viewer() != null && Replays.viewer().status() == null
					&& Minecraft.getInstance().level != null, () -> after(3, () -> {
				shot("replay from launcher");
				Replays.stopWatching();
				com.arcticlauncher.client.ArcticClient.config().replayAutoExport = true;

				when("live MP4 finalized without opening a replay", () -> Replays.queuedVideos() == 0 && Replays.viewer() == null, () -> {
					if (videoSince > 0 && !liveVideosSaved()) { fail("live capture did not save both MP4s" + Replays.liveVideo().status()); return; }
					log("PASSED"); after(2, () -> Minecraft.getInstance().stop());
				});
			}));
			return;
		}
		if ("replaymid".equals(System.getProperty("arctic.selftest"))) {
			// Join a world (Replay off with -Darctic.midoff=true), then start/stop a manual recording as the key does.
			if (Boolean.getBoolean("arctic.midoff")) com.arcticlauncher.client.ArcticClient.config().replayEnabled = false;
			TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(() -> after(12, ReplayTest::mid))), 10, TimeUnit.SECONDS);
			return;
		}
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(ReplayTest::record)), 10, TimeUnit.SECONDS);
	}

	private static void mid() {
		if (Boolean.getBoolean("arctic.midoff")) {
			com.arcticlauncher.client.ArcticClient.config().replayEnabled = true;
			log("mid: replay switched on inside the world");
		}
		log("mid: take=" + (Replays.recorder().current() != null));
		shot("mid before recording");
		Replays.toggleRecording();
		log("mid: recording=" + Replays.recording());
		if (!Replays.recording()) { fail("recording did not start in an existing world"); return; }
		command("time set noon");
		after(8, () -> {
			Replays.toggleRecording();
			when("mid recording saved", () -> Replays.recorder().pendingSaves() == 0 && Replays.folder().listFiles((d, n) -> n.endsWith(".mcpr")) != null
					&& Replays.folder().listFiles((d, n) -> n.endsWith(".mcpr")).length > 0, () -> {
				final File f = Replays.folder().listFiles((d, n) -> n.endsWith(".mcpr"))[0];
				log("mid: saved " + f.getName() + " " + f.length() / 1024 + " KB");
				com.arcticlauncher.client.ArcticClient.platform().leaveWorld();
				when("left the world", () -> Minecraft.getInstance().level == null, () -> after(3, () -> {
					Replays.watch(f);
					when("mid replay opened", () -> Replays.viewer() != null && (Replays.viewer().ready() || Replays.viewer().status() != null), () -> {
						ReplayViewer v = Replays.viewer();
						log("mid: opened ready=" + v.ready() + " status=" + v.status() + " duration=" + v.duration());
						after(4, () -> { shot("mid replay playing"); log(v.ready() && v.duration() > 3000 ? "PASSED" : "FAILED mid replay unusable"); Minecraft.getInstance().stop(); });
					});
				}));
			});
		});
	}

	private static void run(Runnable r) {
		QUEUED.add(() -> {
			try {
				r.run();
			} catch (RuntimeException e) {
				fail("step threw " + e);
				ArcticMod.LOG.error("replaytest", e);
			}
		});
	}

	/** Development actions run outside Minecraft's disposable connection queue. */
	public static void frame() {
		if (inFrame || QUEUED.isEmpty()) return;
		inFrame = true;
		try {
			for (Runnable r; (r = QUEUED.poll()) != null;) r.run();
		} finally {
			inFrame = false;
		}
	}

	private static void log(String message) {
		ArcticMod.LOG.info("replaytest: {}", message);
	}

	private static void fail(String why) {
		ArcticMod.LOG.error("replaytest: FAILED {}", why);
		run(() -> Minecraft.getInstance().stop());
	}

	/** Run {@code then} on the game thread once {@code ready} holds (checked each 250 ms). */
	private static void when(String what, BooleanSupplier ready, Runnable then) {
		long deadline = System.currentTimeMillis() + WAIT_LIMIT_S * 1000L;
		TIMER.schedule(new Runnable() {
			@Override
			public void run() {
				Runnable poll = this;
				ReplayTest.run(() -> {
					if (ready.getAsBoolean()) {
						log(what);
						then.run();
					} else if (System.currentTimeMillis() > deadline) {
						fail("timed out waiting: " + what);
					} else {
						TIMER.schedule(poll, 250, TimeUnit.MILLISECONDS);
					}
				});
			}
		}, 250, TimeUnit.MILLISECONDS);
	}

	private static void after(int seconds, Runnable then) {
		TIMER.schedule(() -> run(then), seconds, TimeUnit.SECONDS);
	}

	// ---- Recording --------------------------------------------------------------

	private static void record() {
		if (!Replays.liveVideo().capturing()) { when("live video capture ready", () -> Replays.liveVideo().capturing(), ReplayTest::record); return; }
		Minecraft mc = Minecraft.getInstance();
		mc.options.pauseOnLostFocus = false;
		if (Replays.recorder().current() == null) {
			fail("nothing is recording in the world");
			return;
		}
		com.arcticlauncher.client.ArcticClient.config().clipSeconds = 3;
		com.arcticlauncher.client.ArcticClient.config().replayAutoExport = false;
		if (!Replays.recorder().startRecording(manual::set)) { fail("manual recording did not start in an existing world"); return; }
		videoSince = System.currentTimeMillis();
		Replays.liveVideo().startRecording();
		log("recording; independent 3s clip buffer enabled");
		command("time set noon");
		command("summon minecraft:pig ~3 ~ ~");
		command("summon minecraft:zombie ~-4 ~ ~2");
		command("summon minecraft:armor_stand ~ ~ ~5");
		for (int i = 0; i < 8; i++) {
			final int step = i;
			after(1 + i, () -> {
				Minecraft m = Minecraft.getInstance();
				if (m.player == null) {
					return;
				}
				m.player.setPos(Compat.x(m.player) + 1.5, Compat.y(m.player), Compat.z(m.player));
				com.arcticlauncher.mod.Compat.setYRot(m.player, step * 30f);
				com.arcticlauncher.mod.replay.ReplayCompat.swing(m.player);
				command("setblock ~ ~-1 ~2 minecraft:gold_block");
			});
		}
		after(11, ReplayTest::keep);
		after(4, () -> command("kill @s"));
		after(6, () -> { if (Minecraft.getInstance().player != null) Minecraft.getInstance().player.respawn(); });
		after(9, () -> command("playsound minecraft:block.note_block.pling master @a ~ ~ ~ 1 1"));
		after(10, () -> command("playsound minecraft:block.note_block.bell master @a ~ ~ ~ 1 1"));
	}

	private static void keep() {
		final AtomicReference<File> saved = new AtomicReference<File>();
		final long since = System.currentTimeMillis() - 1000;
		Replays.saveMoment();
		when("kept a replay", () -> {
			File[] files = Replays.folder().listFiles((d, n) -> n.endsWith(".mcpr"));
			if (files != null) {
				for (File f : files) {
					if (f.lastModified() >= since) {
						saved.set(f);
					}
				}
			}
			return saved.get() != null;
		}, () -> {
			log("thumbnail " + hasThumbnail(saved.get()));
			log("file " + saved.get().getName() + " (" + saved.get().length() / 1024 + " KB)");
			if (!Replays.recording()) { fail("saving a clip stopped manual recording"); return; }
			long clipSize = saved.get().length();
			after(2, () -> {
				if (!Replays.recorder().stopRecording()) { fail("manual recording already stopped"); return; }
				Replays.liveVideo().stopRecording();
				when("manual recording saved separately", () -> manual.get() != null, () -> {
					if (saved.get().equals(manual.get()) || saved.get().length() != clipSize) { fail("clip was changed by the manual recording"); return; }
					com.arcticlauncher.client.ArcticClient.platform().leaveWorld();
					when("left the world", () -> Minecraft.getInstance().level == null
							&& !com.arcticlauncher.mod.replay.ReplayCompat.messageShowing(), () -> after(3, () -> {
						Replays.watch(manual.get());
						when("manual replay opened from mid-world bootstrap", () -> Replays.viewer() != null && Replays.viewer().ready(), () -> {
							if (Replays.viewer().duration() < 12_000) { fail("manual recording was truncated"); return; }
							testRespawn(() -> { Replays.stopWatching(); after(2, () -> watch(saved.get())); });
						});
					}));
				});
			});
		});
	}

	// ---- Watching ----------------------------------------------------------------

	private static void testRespawn(Runnable done) {
		int deadAt = -1, aliveAt = -1, selfId;
		try (com.arcticlauncher.client.replay.ReplayData data = com.arcticlauncher.client.replay.ReplayData.open(manual.get(),
				new File(manual.get().getParentFile(), "respawn-test-cache"))) {
			selfId = data.meta.selfId;
			for (int i = 0; data.selfSample(i) != null; i++) {
				com.arcticlauncher.client.replay.SelfSample sample = data.selfSample(i);
				if (sample.health <= 0 && deadAt < 0) deadAt = sample.time;
				if (sample.health > 0 && deadAt >= 0) { aliveAt = sample.time; break; }
			}
		} catch (java.io.IOException e) { throw new IllegalStateException(e); }
		if (deadAt < 0 || aliveAt <= deadAt) { fail("death/respawn was not recorded"); return; }
		final int alive = aliveAt, id = selfId, dead = deadAt;
		ReplayViewer v = Replays.viewer(); v.setMode(ReplayViewer.Mode.FREE);
		if (!v.paused()) v.togglePause();
		v.seek(deadAt + Math.min(250, (aliveAt - deadAt) / 2));
		when("dead player replay pose", () -> v.ready() && v.status() == null, () -> {
			net.minecraft.world.entity.Entity body = Minecraft.getInstance().level.getEntity(id);
			if (!(body instanceof net.minecraft.world.entity.LivingEntity) || ((net.minecraft.world.entity.LivingEntity) body).getHealth() > 0) {
				fail("recorded death did not reach the stand-in player"); return;
			}
			v.togglePause();
			when("played through respawn", () -> v.time() >= alive + 300 && v.status() == null, () -> {
				if (!v.paused()) v.togglePause();
				if (!aliveBody(id)) return;
				v.seek(dead);
				when("rewound across respawn", () -> v.status() == null, () -> {
					v.seek(alive + 300);
					when("sought forward across respawn", () -> v.status() == null, () -> {
						if (!aliveBody(id)) return;
						log("death and respawn pose passed during playback and seeking"); done.run();
					});
				});
			});
		});
	}
	private static boolean aliveBody(int id) {
		net.minecraft.world.entity.Entity entity = Minecraft.getInstance().level.getEntity(id);
		if (!(entity instanceof net.minecraft.world.entity.LivingEntity)) { fail("missing respawned player model"); return false; }
		net.minecraft.world.entity.LivingEntity body = (net.minecraft.world.entity.LivingEntity) entity;
		if (body.getHealth() <= 0 || body.deathTime != 0) { fail("respawned player still has the death pose"); return false; }
		return true;
	}

	private static void watch(File file) {
		Replays.watch(file);
		when("watching", () -> Replays.viewer() != null && Replays.viewer().status() == null
				&& Minecraft.getInstance().level != null, ReplayTest::tourReplay);
	}

	private static void tourReplay() {
		final ReplayViewer v = Replays.viewer();
		log("duration " + v.duration() + " ms, fast jumps " + v.fastJumps());
		after(3, () -> {
			shot("replay start");
			v.seek(v.duration() * 0.8);
			when("jumped forward", () -> v.status() == null && v.time() >= v.duration() * 0.79, () -> after(2, () -> {
				shot("replay jumped forward");
				List<Object[]> players = v.followable();
				log("followable " + players.size());
				// From the timeline's start: the first world frame comes a moment into the recording.
				v.seek(v.start() + 1000);
				when("jumped back", () -> v.status() == null && v.time() < v.start() + 2500 && Minecraft.getInstance().level != null, () -> after(3, () -> {
					shot("replay jumped back");
					if (!testSeekView()) return;
					v.setMode(ReplayViewer.Mode.FIRST_PERSON);
					after(2, () -> {
						shot("replay first person");
						testPath(() -> {
							shot("replay path");
							com.arcticlauncher.client.ArcticClient.platform().openPage(new ReplayMenu());
							after(2, () -> { shot("replay menu"); grabFrame(); });
						});
					});
				}));
			}));
		});
	}

	/** Repeated paused rewinds must keep the editor, camera and fresh world's connection. */
	private static boolean testSeekView() {
		ReplayViewer v = Replays.viewer();
		v.setMode(ReplayViewer.Mode.FREE);
		if (!v.paused()) v.togglePause();
		double[] camera = com.arcticlauncher.client.ArcticClient.platform().camera();
		com.arcticlauncher.mod.replay.ReplayPlayback.INSTANCE.camera(camera[0], camera[1], camera[2], 37, -12, 0, false);
		com.arcticlauncher.client.ArcticClient.platform().openPage(new ReplayMenu());
		net.minecraft.client.gui.screens.Screen editor = Compat.screen();
		for (int i = 0; i < 3; i++) {
			v.seek(v.duration() * 0.8); Replays.frame(true);
			net.minecraft.client.multiplayer.ClientLevel old = Minecraft.getInstance().level;
			v.seek(v.start() + 250); Replays.frame(true);
			float[] look = com.arcticlauncher.mod.replay.ReplayPlayback.INSTANCE.look();
			if (!v.ready() || v.status() != null || !v.paused() || Compat.screen() != editor
					|| Minecraft.getInstance().level == old || Minecraft.getInstance().player.connection != Minecraft.getInstance().getConnection()
					|| Math.abs(v.time() - v.start() - 250) > 1 || Math.abs(look[0] - 37) > 0.1 || Math.abs(look[1] + 12) > 0.1) {
				fail("rewind lost its editor, camera, clock or rebuilt world"); return false;
			}
		}
		Compat.setScreen(null);
		v.togglePause();
		log("repeated paused rewinds kept editor and camera without a loading screen");
		return true;
	}

	/** Keyframe editing while paused must not trap the camera behind a closed menu. */
	private static void testPath(Runnable done) {
		final ReplayViewer v = Replays.viewer();
		v.setMode(ReplayViewer.Mode.FREE);
		if (!v.paused()) v.togglePause();
		v.clearKeyframes();
		v.seek(v.start() + 250);
		when("ready for paused keyframe", () -> v.ready() && v.status() == null, () -> {
			v.addKeyframe(); v.addKeyframe();
			if (v.keyframes().size() != 1) { fail("paused keyframe was duplicated"); return; }
			com.arcticlauncher.client.ArcticClient.platform().openPage(new ReplayMenu());
			after(1, () -> {
				Compat.setScreen(null);
				double[] before = com.arcticlauncher.client.ArcticClient.platform().camera();
				Compat.setDown(Minecraft.getInstance().options.keyUp, true);
				after(1, () -> {
					Compat.setDown(Minecraft.getInstance().options.keyUp, false);
					double[] at = com.arcticlauncher.client.ArcticClient.platform().camera();
					double moved = Math.abs(at[0] - before[0]) + Math.abs(at[1] - before[1]) + Math.abs(at[2] - before[2]);
					if (moved < 0.5 || !v.paused()) { fail("camera stuck after closing paused keyframe editor"); return; }
					v.seek(v.start() + 1250);
					when("second keyframe time", () -> v.ready() && v.status() == null, () -> {
						v.addKeyframe();
						if (!v.canPlayPath()) { fail("two distinct keyframes did not make a path"); return; }
						v.playPath();
						when("edited path completed", () -> v.mode() == ReplayViewer.Mode.FREE, () -> {
							log("paused keyframe camera and path controls passed"); done.run();
						});
					});
				});
			});
		});
	}

	/** The export path: one frame read back from the GPU. */
	private static void grabFrame() {
		final AtomicReference<String> got = new AtomicReference<String>();
		com.arcticlauncher.mod.replay.ReplayPlayback.INSTANCE.capture(new ReplayBackend.FrameSink() {
			@Override
			public void frame(int width, int height, ByteBuffer rgba, boolean bottomUp) {
				got.set(width + "x" + height + " " + rgba.remaining() + " bytes");
			}
		});
		when("grabbed a frame", () -> got.get() != null, () -> {
			log("frame " + got.get());
			Compat.setScreen(null);
			when("found FFmpeg", () -> Replays.ffmpeg().get() != null, ReplayTest::export);
		});
	}

	/** A 3-second first-person clip through FFmpeg, like the Export clip button. */
	private static void export() {
		final ReplayViewer v = Replays.viewer();
		File out = new File(Replays.videoFolder(), "replaytest.mp4");
		out.delete();
		int end = (int) Math.min(v.duration(), v.start() + 6000);
		final com.arcticlauncher.client.replay.VideoExport job = new com.arcticlauncher.client.replay.VideoExport(
				Replays.ffmpeg().get(), out, end - 3000, end, 30, null);
		long started = System.nanoTime();
		v.export(job, ReplayViewer.Mode.FIRST_PERSON);
		when("exported", job::done, () -> {
			log("export took " + (System.nanoTime() - started) / 1_000_000 + " ms, error " + job.error()
					+ ", file " + out.length() / 1024 + " KB");
			if (job.error() != null || out.length() == 0) {
				fail("export " + job.error());
				return;
			}
			Replays.stopWatching();
			when("back at the title", () -> Minecraft.getInstance().level == null && Replays.viewer() == null, () -> {
				com.arcticlauncher.client.ArcticClient.config().replayAutoExport = true;

				when("live MP4 finalized without opening a replay", () -> Replays.queuedVideos() == 0 && Replays.viewer() == null, () -> {
					if (videoSince > 0 && !liveVideosSaved()) { fail("live capture did not save both MP4s" + Replays.liveVideo().status()); return; }
					log("PASSED"); after(2, () -> Minecraft.getInstance().stop());
				});
			});
		});
	}

	private static boolean hasThumbnail(File replay) {
		try (java.util.zip.ZipFile zip = new java.util.zip.ZipFile(replay)) {
			return zip.getEntry("arctic/thumb.png") != null;
		} catch (java.io.IOException e) {
			return false;
		}
	}

	private static void command(String command) {
		Compat.sendChat("/" + command);
	}

	private static void shot(String name) {
		WorldTest.shot(name);
		log("screenshot " + name);
	}
	private static boolean liveVideosSaved() {
		File[] files = Replays.videoFolder().listFiles((d, name) -> name.endsWith(".mp4") && (name.startsWith("Clip ") || name.startsWith("Recording ")));
		int found = 0;
		if (files != null) for (File file : files) if (file.lastModified() >= videoSince && file.length() > 512) found++;
		return found >= 2;
	}

	// ---- Load-time check (-Darctic.selftest=replayperf / replayperfopen) ------------------------------

	/** Longest gap between two game-thread frames while measuring (ms): how long the game froze. */
	private static volatile long stallMs;
	private static volatile long lastBeat;
	private static volatile boolean beating;

	private static void beat() {
		long now = System.nanoTime();
		if (lastBeat != 0) stallMs = Math.max(stallMs, (now - lastBeat) / 1_000_000);
		lastBeat = now;
		if (beating) TIMER.schedule(() -> run(ReplayTest::beat), 1, TimeUnit.MILLISECONDS);
	}

	private static void startStallWatch() {
		stallMs = 0;
		lastBeat = 0;
		if (!beating) {
			beating = true;
			beat();
		}
	}

	/** A long play session: teleport between far areas so the server streams chunks over and over. */
	private static void perfSession() {
		int rounds = Integer.getInteger("arctic.perfrounds", 40);
		Minecraft.getInstance().options.pauseOnLostFocus = false;
		com.arcticlauncher.client.ArcticClient.config().clipSeconds = 10;
		com.arcticlauncher.client.ArcticClient.config().replayAutoExport = false;
		log("perf: session of " + rounds + " teleports");
		command("gamerule doDaylightCycle false");
		command("summon villager ~2 ~ ~2");
		for (int i = 0; i < rounds; i++) {
			int round = i;
			after(2 + i * 3, () -> command("tp @p " + (round % 2 == 0 ? 3000 : 0) + " 120 " + (round % 4 < 2 ? 0 : 3000)));
		}
		after(2 + rounds * 3 + 5, () -> {
			long since = System.currentTimeMillis() - 1000;
			AtomicReference<File> saved = new AtomicReference<File>();
			Replays.saveMoment();
			when("perf: clip kept", () -> {
				File[] files = Replays.folder().listFiles((d, n) -> n.endsWith(".mcpr"));
				if (files != null) for (File f : files) if (f.lastModified() >= since) saved.set(f);
				return saved.get() != null && Replays.pendingSaves() == 0;
			}, () -> {
				log("perf: clip " + saved.get().getAbsolutePath() + " " + saved.get().length() / 1024 + " KB");
				com.arcticlauncher.client.ArcticClient.platform().leaveWorld();
				when("left the world", () -> Minecraft.getInstance().level == null, () -> after(3, () -> perfOpen(saved.get())));
			});
		});
	}

	private static void perfOpen(File file) {
		log("perf: opening " + file.getName() + " (" + file.length() / 1024 + " KB)");
		startStallWatch();
		long begun = System.currentTimeMillis();
		Replays.watch(file);
		when("perf: world showing", () -> Replays.viewer() != null && Replays.viewer().ready() && Replays.viewer().status() == null
				&& Minecraft.getInstance().level != null, () -> {
			long openMs = System.currentTimeMillis() - begun;
			long openStall = stallMs;
			after(3, () -> {
				shot("perf replay open");
				ReplayViewer v = Replays.viewer();
				log("perf: RESULT open_ms=" + openMs + " open_max_frame_ms=" + openStall + " duration_ms=" + v.duration()
						+ " fast_jumps=" + v.fastJumps());
				stallMs = 0;
				long f0 = System.currentTimeMillis();
				v.seek(v.duration() * 0.9);
				when("perf: jumped forward", () -> v.status() == null && v.ready(), () -> {
					long fwd = System.currentTimeMillis() - f0;
					long fwdStall = stallMs;
					after(2, () -> {
						stallMs = 0;
						long b0 = System.currentTimeMillis();
						v.seek(v.start());
						when("perf: jumped back", () -> v.status() == null && v.ready() && Minecraft.getInstance().level != null, () -> {
							log("perf: RESULT seek_forward_ms=" + fwd + " seek_forward_max_frame_ms=" + fwdStall
									+ " seek_back_ms=" + (System.currentTimeMillis() - b0) + " seek_back_max_frame_ms=" + stallMs);
							after(2, () -> {
								shot("perf replay after seeks");
								beating = false;
								Replays.stopWatching();
								log("PASSED");
								after(2, () -> Minecraft.getInstance().stop());
							});
						});
					});
				});
			});
		});
	}

	// ---- Audit phases (-Darctic.selftest=replayaudit; see mod/tools/replayaudit) ---------------------
	// Timed phases with nothing else going on, so an outside sampler can attribute cost to each:
	// off | buffer -> manual -> video -> open -> playback -> seeks -> paused. Lines are "audit: ... @epochMs".

	private static void alog(String message) {
		log("audit: " + message + " @" + System.currentTimeMillis());
	}

	private static void phase(final String name, int seconds, final Runnable then) {
		alog("PHASE " + name + " BEGIN");
		after(seconds, () -> {
			alog("PHASE " + name + " END");
			then.run();
		});
	}

	private static File newestReplay(String kind) {
		File[] files = Replays.folder().listFiles((d, n) -> n.endsWith(".mcpr") && n.contains(kind));
		File best = null;
		if (files != null) for (File f : files) if (best == null || f.lastModified() > best.lastModified()) best = f;
		return best;
	}

	private static void auditStart() {
		final boolean off = Boolean.getBoolean("arctic.audit.off");
		com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		c.replayEnabled = !off;
		c.replayVideoEnabled = false;
		c.replayAutoExport = false;
		c.clipSeconds = 30;
		alog("scheduled off=" + off);
		final Runnable go = Boolean.getBoolean("arctic.audit.render") ? () -> after(6, AUDIT_RENDER) : () -> after(15, off ? AUDIT_OFF : AUDIT_ON);
		if (Boolean.getBoolean("arctic.audit.server")) {
			TIMER.schedule(() -> run(() -> when("audit: joined the server", AUDIT_IN_WORLD, go)), 5, TimeUnit.SECONDS);
		} else {
			TIMER.schedule(() -> run(() -> AUDIT_CREATE.accept(go)), 10, TimeUnit.SECONDS);
		}
	}

	/** Replay off at the join: the baseline, then switching it on and recording inside the world. */
	private static void auditOff() {
		final int seconds = Integer.getInteger("arctic.audit.phase", 40);
		alog("in world; take=" + (Replays.recorder().current() != null));
		phase("off", seconds, () -> {
			AUDIT_SHOT.accept("audit live");
			com.arcticlauncher.client.ArcticClient.config().replayEnabled = true;
			after(2, () -> {
				alog("switched on in world; take=" + (Replays.recorder().current() != null) + " bufferReady=" + Replays.recorder().bufferReady());
				Replays.toggleRecording();
				alog("INWORLD recording=" + Replays.recording() + " bufferReady=" + Replays.recorder().bufferReady());
				after(3, () -> {
					alog("INWORLD 3 s later recording=" + Replays.recording() + " bufferReady=" + Replays.recorder().bufferReady()
							+ " status=" + Replays.recordingStatus());
					Replays.saveMoment();
					after(10, () -> {
						if (Replays.recording()) Replays.toggleRecording();
						when("audit: in-world saves settled", () -> Replays.pendingSaves() == 0, () -> after(2, () -> {
							File clip = newestReplay("Clip");
							File rec = newestReplay("Recording");
							alog("INWORLD files clip=" + (clip == null ? "none" : clip.length() / 1024 + "KB") + " recording="
									+ (rec == null ? "none" : rec.length() / 1024 + "KB"));
							auditOpen(rec != null ? rec : clip, 8, false);
						}));
					});
				});
			});
		});
	}

	/** Replay on from the join: buffer only, then a manual recording, then gameplay video on top. */
	private static void auditOn() {
		final int seconds = Integer.getInteger("arctic.audit.phase", 40);
		final com.arcticlauncher.client.config.ClientConfig c = com.arcticlauncher.client.ArcticClient.config();
		alog("in world; take=" + (Replays.recorder().current() != null) + " bufferReady=" + Replays.recorder().bufferReady());
		phase("buffer", seconds, () -> {
			AUDIT_SHOT.accept("audit live");
			Replays.toggleRecording();
			alog("manual recording=" + Replays.recording());
			// An instant clip (F9) in the middle of the manual recording: its hitch shows in the frame log.
			after(seconds / 2, () -> { alog("MARK clip"); Replays.saveMoment(); });
			phase("manual", seconds, () -> {
				final Runnable finish = () -> {
					c.replayVideoEnabled = false;
					if (Replays.recording()) Replays.toggleRecording();
					final long asked = System.currentTimeMillis();
					when("audit: recording saved", () -> Replays.pendingSaves() == 0 && newestReplay("Recording") != null, () -> {
						File rec = newestReplay("Recording");
						alog("SAVED recording " + rec.length() / 1024 + "KB within " + (System.currentTimeMillis() - asked) + " ms");
						auditOpen(rec, Math.min(seconds, 40), true);
					});
				};
				if (Boolean.getBoolean("arctic.audit.novideo")) { finish.run(); return; }
				c.replayVideoEnabled = true;
				final long deadline = System.currentTimeMillis() + 25_000;
				when("audit: video capture decided", () -> Replays.liveVideo().capturing() || System.currentTimeMillis() > deadline, () -> {
					alog("video capturing=" + Replays.liveVideo().capturing() + " status=" + Replays.liveVideo().status());
					if (!Replays.liveVideo().capturing()) { finish.run(); return; }
					phase("video", seconds, finish);
				});
			});
		});
	}

	private static void auditOpen(final File file, final int seconds, final boolean seeks) {
		if (file == null) { alog("OPEN nothing to open"); AUDIT_QUIT.run(); return; }
		com.arcticlauncher.client.ArcticClient.platform().leaveWorld();
		when("audit: left the world", AUDIT_NO_WORLD, () -> after(3, () -> {
			final long begun = System.currentTimeMillis();
			alog("OPEN BEGIN " + file.getName() + " " + file.length() / 1024 + "KB");
			Replays.watch(file);
			final long deadline = System.currentTimeMillis() + 60_000;
			when("audit: open decided", () -> (Replays.viewer() != null && Replays.viewer().ready() && Replays.viewer().status() == null)
					|| System.currentTimeMillis() > deadline, () -> {
				final ReplayViewer v = Replays.viewer();
				alog("OPEN END ready=" + (v != null && v.ready()) + " status=" + (v == null ? "no viewer" : v.status()) + " screen=" + AUDIT_SCREEN.get()
						+ " ms=" + (System.currentTimeMillis() - begun) + " duration=" + (v == null ? -1 : v.duration()));
				if (v == null || !v.ready()) {
					AUDIT_SHOT.accept("audit open stuck");
					after(2, AUDIT_QUIT);
					return;
				}
				phase("playback", seconds, () -> {
					AUDIT_SHOT.accept("audit playback");
					if (!seeks) { after(2, AUDIT_QUIT); return; }
					auditSeek(v, new double[] {0.9, 0.0, 0.5, 0.1, 0.95, 0.0}, 0, () -> {
						if (!v.paused()) v.togglePause();
						phase("paused", 10, () -> after(1, AUDIT_QUIT));
					});
				});
			});
		}));
	}

	/**
	 * "Render" from the Replays tab on a clip saved seconds after joining: the export window starts at the
	 * replay's first moment, before the recorded position packet that ends the terrain screen.
	 */
	private static void auditRender() {
		alog("RENDER in world; take=" + (Replays.recorder().current() != null));
		Replays.saveMoment();
		when("audit: early clip saved", () -> Replays.pendingSaves() == 0 && newestReplay("Clip") != null, () -> {
			final File clip = newestReplay("Clip");
			com.arcticlauncher.client.ArcticClient.platform().leaveWorld();
			when("audit: left the world", AUDIT_NO_WORLD, () -> after(3, () -> {
				final boolean render = !Boolean.getBoolean("arctic.audit.watchonly");
				alog("RENDER opening " + clip.getName() + " render=" + render);
				if (render) Replays.watch(clip, v -> ReplayMenu.exportClip(v, -1));
				else Replays.watch(clip);
				auditRenderPoll(0);
			}));
		});
	}

	private static void auditRenderPoll(final int n) {
		after(2, () -> {
			ReplayViewer v = Replays.viewer();
			alog("RENDER t+" + (n * 2 + 2) + "s viewer=" + (v != null) + " ready=" + (v != null && v.ready()) + " screen=" + AUDIT_SCREEN.get()
					+ " exporting=" + (v != null && v.exporting() != null) + " time=" + (v == null ? -1 : (long) v.time())
					+ " start=" + (v == null ? -1 : v.start()));
			if (n >= 14) {
				AUDIT_SHOT.accept("audit render end");
				alog("RENDER RESULT " + (v != null && !v.ready() ? "STUCK on " + AUDIT_SCREEN.get() : "not stuck"));
				after(1, AUDIT_QUIT);
			} else auditRenderPoll(n + 1);
		});
	}

	private static void auditSeek(final ReplayViewer v, final double[] targets, final int i, final Runnable then) {
		if (i >= targets.length) { then.run(); return; }
		final double target = v.start() + (v.duration() - v.start()) * targets[i];
		alog("SEEK BEGIN to=" + targets[i] + " from_ms=" + (long) v.time() + " to_ms=" + (long) target);
		v.seek(target);
		when("audit: seek settled", () -> v.status() == null && v.ready(), () -> {
			alog("SEEK END to=" + targets[i]);
			after(3, () -> auditSeek(v, targets, i + 1, then));
		});
	}

	private static final Runnable AUDIT_RENDER = ReplayTest::auditRender;
	private static final Runnable AUDIT_OFF = ReplayTest::auditOff;
	private static final Runnable AUDIT_ON = ReplayTest::auditOn;
	private static final BooleanSupplier AUDIT_IN_WORLD = () -> Minecraft.getInstance().level != null && Minecraft.getInstance().player != null;
	private static final BooleanSupplier AUDIT_NO_WORLD = () -> Minecraft.getInstance().level == null;
	private static final java.util.function.Consumer<Runnable> AUDIT_CREATE = WorldTest::createTestWorld;
	private static final java.util.function.Consumer<String> AUDIT_SHOT = ReplayTest::shot;
	private static final java.util.function.Supplier<String> AUDIT_SCREEN = () -> { Object s = Compat.screen(); return s == null ? "none" : s.getClass().getSimpleName(); };
	private static final Runnable AUDIT_QUIT = () -> { alog("DONE"); Minecraft.getInstance().stop(); };
}
//#endif
