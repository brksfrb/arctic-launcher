package com.arcticlauncher.legacy;

import com.arcticlauncher.client.replay.ReplayBackend;
import com.arcticlauncher.client.replay.ReplayMenu;
import com.arcticlauncher.client.replay.ReplayViewer;
import com.arcticlauncher.client.replay.Replays;
import com.arcticlauncher.client.replay.VideoExport;
import com.arcticlauncher.legacy.replay.LegacyReplayPlayback;
import java.io.File;
import java.nio.ByteBuffer;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import net.minecraft.client.MinecraftClient;

/**
 * Development check, only with {@code -Darctic.selftest=replay}: the twin
 * of the modern ReplayTest on old versions (record, keep, leave, watch,
 * jump both ways, first person, a camera path, the menu, a frame grab and
 * a short video), logging "replaytest: ...".
 */
final class LegacyReplayTest {
	private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
		Thread t = new Thread(r, "arctic-replaytest");
		t.setDaemon(true);
		return t;
	});
	private static final int WAIT_LIMIT_S = 120;

	private LegacyReplayTest() {}

	static boolean requested() {
		return "replay".equals(System.getProperty("arctic.selftest"));
	}

	static void start() {
		log("scheduled");
		TIMER.schedule(() -> run(() -> LegacyWorldTest.createTestWorld(LegacyReplayTest::record)), 10, TimeUnit.SECONDS);
	}

	private static void run(Runnable r) {
		LegacyHooks.onNextFrame(() -> {
			try {
				r.run();
			} catch (RuntimeException e) {
				ArcticLegacy.LOG.error("replaytest: FAILED step threw", e);
				MinecraftClient.getInstance().scheduleStop();
			}
		});
	}

	private static void log(String message) {
		ArcticLegacy.LOG.info("replaytest: " + message);
	}

	private static void fail(String why) {
		ArcticLegacy.LOG.error("replaytest: FAILED " + why);
		MinecraftClient.getInstance().scheduleStop();
	}

	interface Check {
		boolean ok();
	}

	/** Run {@code then} on the game thread once {@code ready} holds (checked on the game thread). */
	private static void when(final String what, final Check ready, final Runnable then) {
		final long deadline = System.currentTimeMillis() + WAIT_LIMIT_S * 1000L;
		TIMER.schedule(new Runnable() {
			@Override
			public void run() {
				final Runnable self = this;
				LegacyReplayTest.run(() -> {
					if (ready.ok()) {
						log(what);
						then.run();
					} else if (System.currentTimeMillis() > deadline) {
						fail("timed out waiting: " + what);
					} else {
						TIMER.schedule(self, 250, TimeUnit.MILLISECONDS);
					}
				});
			}
		}, 250, TimeUnit.MILLISECONDS);
	}

	private static void after(int seconds, Runnable then) {
		TIMER.schedule(() -> run(then), seconds, TimeUnit.SECONDS);
	}

	private static void record() {
		if (Replays.recorder().current() == null) {
			fail("nothing is recording in the world");
			return;
		}
		log("recording");
		LegacyWorldTest.command("time set 6000");
		for (int i = 0; i < 8; i++) {
			after(1 + i, () -> {
				MinecraftClient m = MinecraftClient.getInstance();
				if (m.player == null) {
					return;
				}
				m.player.updatePosition(m.player.x + 1.5, m.player.y, m.player.z);
				m.player.yaw += 30;
				//#if MC >= 1.9
				m.player.swingHand(net.minecraft.util.Hand.MAIN_HAND);
				//#else
				m.player.swingHand();
				//#endif
				LegacyWorldTest.command("setblock ~ ~-1 ~2 minecraft:gold_block");
			});
		}
		after(11, LegacyReplayTest::keep);
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
			log("file " + saved.get().getName() + " (" + saved.get().length() / 1024 + " KB)");
			com.arcticlauncher.client.ArcticClient.platform().leaveWorld();
			when("left the world", () -> MinecraftClient.getInstance().world == null, () -> after(3, () -> watch(saved.get())));
		});
	}

	private static void watch(File file) {
		Replays.watch(file);
		when("watching", () -> Replays.viewer() != null && Replays.viewer().status() == null
				&& MinecraftClient.getInstance().world != null && MinecraftClient.getInstance().currentScreen == null, LegacyReplayTest::tour);
	}

	private static void tour() {
		final ReplayViewer v = Replays.viewer();
		log("duration " + v.duration() + " ms, fast jumps " + v.fastJumps());
		after(3, () -> {
			LegacyWorldTest.shot("replay start");
			v.seek(v.duration() * 0.8);
			when("jumped forward", () -> v.status() == null && v.time() >= v.duration() * 0.79, () -> after(2, () -> {
				LegacyWorldTest.shot("replay jumped forward");
				log("followable " + v.followable().size());
				v.seek(0);
				when("jumped back", () -> v.status() == null && v.time() < v.start() + 2500
						&& MinecraftClient.getInstance().world != null, () -> after(3, () -> {
					LegacyWorldTest.shot("replay jumped back");
					v.setMode(ReplayViewer.Mode.FIRST_PERSON);
					after(2, () -> {
						LegacyWorldTest.shot("replay first person");
						v.setMode(ReplayViewer.Mode.FREE);
						v.addKeyframe();
						v.jump(2000);
						after(2, () -> {
							v.addKeyframe();
							v.playPath();
							after(2, () -> {
								LegacyWorldTest.shot("replay path");
								com.arcticlauncher.client.ArcticClient.platform().openPage(new ReplayMenu());
								after(2, () -> {
									LegacyWorldTest.shot("replay menu");
									grabFrame();
								});
							});
						});
					});
				}));
			}));
		});
	}

	private static void grabFrame() {
		final AtomicReference<String> got = new AtomicReference<String>();
		LegacyReplayPlayback.INSTANCE.capture(new ReplayBackend.FrameSink() {
			@Override
			public void frame(int width, int height, ByteBuffer rgba, boolean bottomUp) {
				got.set(width + "x" + height + " " + rgba.remaining() + " bytes");
			}
		});
		when("grabbed a frame", () -> got.get() != null, () -> {
			log("frame " + got.get());
			MinecraftClient.getInstance().setScreen(null);
			when("found FFmpeg", () -> Replays.ffmpeg().get() != null, LegacyReplayTest::export);
		});
	}

	private static void export() {
		final ReplayViewer v = Replays.viewer();
		final File out = new File(Replays.videoFolder(), "replaytest.mp4");
		out.delete();
		int end = (int) Math.min(v.duration(), v.start() + 6000);
		final VideoExport job = new VideoExport(Replays.ffmpeg().get(), out, end - 3000, end, 30, null);
		final long started = System.nanoTime();
		v.export(job, ReplayViewer.Mode.FIRST_PERSON);
		when("exported", job::done, () -> {
			log("export took " + (System.nanoTime() - started) / 1_000_000 + " ms, error " + job.error()
					+ ", file " + out.length() / 1024 + " KB");
			if (job.error() != null || out.length() == 0) {
				fail("export " + job.error());
				return;
			}
			Replays.stopWatching();
			when("back at the title", () -> MinecraftClient.getInstance().world == null && Replays.viewer() == null, () -> {
				log("PASSED");
				after(2, () -> MinecraftClient.getInstance().scheduleStop());
			});
		});
	}
}
