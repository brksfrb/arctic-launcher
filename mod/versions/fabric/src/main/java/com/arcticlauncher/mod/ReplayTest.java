//#if MC >= 1.15
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
	private static final int WAIT_LIMIT_S = 120;
	private static final AtomicReference<File> manual = new AtomicReference<File>();

	private ReplayTest() {}

	static boolean requested() {
		String mode = System.getProperty("arctic.selftest");
		return "replay".equals(mode) || "replayopen".equals(mode);
	}

	static void start() {
		log("scheduled");
		if ("replayopen".equals(System.getProperty("arctic.selftest"))) {
			// Launched with a replay to open (arctic launch --replay): just check it opens.
			when("opened from the launcher", () -> Replays.viewer() != null && Replays.viewer().status() == null
					&& Minecraft.getInstance().level != null, () -> after(3, () -> {
				shot("replay from launcher");
				Replays.stopWatching();
				com.arcticlauncher.client.ArcticClient.config().replayAutoExport = true;
				Replays.resumeExports();
				when("queued MP4 exported after leaving", () -> Replays.queuedVideos() == 0 && Replays.viewer() == null, () -> {
					log("PASSED"); after(2, () -> Minecraft.getInstance().stop());
				});
			}));
			return;
		}
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(ReplayTest::record)), 10, TimeUnit.SECONDS);
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
		Minecraft mc = Minecraft.getInstance();
		mc.options.pauseOnLostFocus = false;
		if (Replays.recorder().current() == null) {
			fail("nothing is recording in the world");
			return;
		}
		com.arcticlauncher.client.ArcticClient.config().clipSeconds = 3;
		com.arcticlauncher.client.ArcticClient.config().replayAutoExport = false;
		if (!Replays.recorder().startRecording(manual::set)) { fail("manual recording did not start in an existing world"); return; }
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
				m.player.setPos(m.player.getX() + 1.5, m.player.getY(), m.player.getZ());
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
				Minecraft.getInstance().options.keyUp.setDown(true);
				after(1, () -> {
					Minecraft.getInstance().options.keyUp.setDown(false);
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
				Replays.resumeExports();
				when("queued MP4 exported after leaving", () -> Replays.queuedVideos() == 0 && Replays.viewer() == null, () -> {
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
}
//#endif
