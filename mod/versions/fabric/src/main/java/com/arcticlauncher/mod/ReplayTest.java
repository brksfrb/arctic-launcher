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
final class ReplayTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-replaytest");
		t.setDaemon(true);
		return t;
	});
	private static final int WAIT_LIMIT_S = 120;

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
				log("PASSED");
				after(2, () -> Minecraft.getInstance().stop());
			}));
			return;
		}
		TIMER.schedule(() -> run(() -> WorldTest.createTestWorld(ReplayTest::record)), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable r) {
		Minecraft.getInstance().execute(() -> {
			try {
				r.run();
			} catch (RuntimeException e) {
				fail("step threw " + e);
				ArcticMod.LOG.error("replaytest", e);
			}
		});
	}

	private static void log(String message) {
		ArcticMod.LOG.info("replaytest: {}", message);
	}

	private static void fail(String why) {
		ArcticMod.LOG.error("replaytest: FAILED {}", why);
		Minecraft.getInstance().execute(() -> Minecraft.getInstance().stop());
	}

	/** Run {@code then} on the game thread once {@code ready} holds (checked each 250 ms). */
	private static void when(String what, BooleanSupplier ready, Runnable then) {
		long deadline = System.currentTimeMillis() + WAIT_LIMIT_S * 1000L;
		TIMER.schedule(new Runnable() {
			@Override
			public void run() {
				AtomicReference<Boolean> ok = new AtomicReference<Boolean>();
				Minecraft.getInstance().executeBlocking(() -> ok.set(ready.getAsBoolean()));
				if (Boolean.TRUE.equals(ok.get())) {
					log(what);
					ReplayTest.run(then);
				} else if (System.currentTimeMillis() > deadline) {
					fail("timed out waiting: " + what);
				} else {
					TIMER.schedule(this, 250, TimeUnit.MILLISECONDS);
				}
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
		log("recording");
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
			com.arcticlauncher.client.ArcticClient.platform().leaveWorld();
			when("left the world", () -> Minecraft.getInstance().level == null
					&& !com.arcticlauncher.mod.replay.ReplayCompat.messageShowing(), () -> after(3, () -> watch(saved.get())));
		});
	}

	// ---- Watching ----------------------------------------------------------------

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
					v.setMode(ReplayViewer.Mode.FIRST_PERSON);
					after(2, () -> {
						shot("replay first person");
						v.setMode(ReplayViewer.Mode.FREE);
						v.addKeyframe();
						v.jump(2000);
						after(2, () -> {
							v.addKeyframe();
							v.playPath();
							after(2, () -> {
								shot("replay path");
								Compat.setScreen(null);
								com.arcticlauncher.client.ArcticClient.platform().openPage(new ReplayMenu());
								after(2, () -> {
									shot("replay menu");
									grabFrame();
								});
							});
						});
					});
				}));
			}));
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
				log("PASSED");
				after(2, () -> Minecraft.getInstance().stop());
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
