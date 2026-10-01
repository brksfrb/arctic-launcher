package com.arcticlauncher.legacy;

import java.text.SimpleDateFormat;
import java.util.Date;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.Features;
import com.arcticlauncher.client.feature.Mentions;
import com.arcticlauncher.client.feature.Streamer;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.util.DefaultSkinHelper;
import net.minecraft.entity.Entity;
import net.minecraft.text.LiteralText;
import net.minecraft.text.Text;
import net.minecraft.util.Identifier;

/** What the 1.8.9 hooks call: small, so each mixin stays a one-liner. */
public final class LegacyHooks {
	/** The skin streamer mode shows for you. */
	public static final Identifier STEVE = DefaultSkinHelper.getTexture();

	/** The player's real angles while the camera borrows Freelook's. */
	private static float[] savedAngles;

	/** The world's field of view last frame (degrees). */
	public static volatile float worldFov = 70f;
	/** The camera last frame: {x, y, z, yaw, pitch}; null before the first. */
	private static volatile double[] camera;

	private LegacyHooks() {}

	/** Where the camera is this frame (called as the world's camera is set up). */
	public static void noteCamera(float t) {
		net.minecraft.entity.Entity e = MinecraftClient.getInstance().getCameraEntity();
		if (e == null) {
			camera = null;
			return;
		}
		camera = new double[] {
				e.prevX + (e.x - e.prevX) * t, e.prevY + (e.y - e.prevY) * t + e.getEyeHeight(), e.prevZ + (e.z - e.prevZ) * t,
				e.prevYaw + (e.yaw - e.prevYaw) * t, e.prevPitch + (e.pitch - e.prevPitch) * t,
		};
	}

	/** {x, y, z, yaw, pitch, fov} for world markers, or null. */
	static double[] camera() {
		double[] c = camera;
		if (c == null || MinecraftClient.getInstance().world == null) {
			return null;
		}
		return new double[] {c[0], c[1], c[2], c[3], c[4], worldFov};
	}

	/** Every client tick. */
	/**
	 * Work for the game thread, run at the start of the next frame (frames
	 * go on when ticks don't, like in a paused replay).
	 * Minecraft's own task queue holds a lock while its tasks run, and the
	 * network thread needs that lock too: a task that leaves a world (and waits
	 * for the connection to close) would never finish.
	 */
	private static final java.util.Queue<Runnable> QUEUED = new java.util.concurrent.ConcurrentLinkedQueue<Runnable>();

	public static void onNextFrame(Runnable r) {
		QUEUED.add(r);
	}

	/** Each frame, before anything else. */
	public static void runQueued() {
		for (Runnable r; (r = QUEUED.poll()) != null;) {
			try {
				r.run();
			} catch (RuntimeException e) {
				ArcticLegacy.LOG.error("a game-thread task failed", e);
			}
		}
	}

	public static void tick(boolean screenOpen) {
		ArcticLegacy.PLATFORM.tick();
		com.arcticlauncher.legacy.replay.LegacyReplayRecording.tick();
		ArcticClient.tick(screenOpen);
	}

	/** Over the game, after Minecraft's own HUD. */
	public static void renderHud() {
		LegacyGfx g = new LegacyGfx();
		ArcticClient.renderHud(g);
		if (ownCrosshair() && MinecraftClient.getInstance().options.perspective == 0) {
			ArcticClient.renderCrosshair(g);
		}
	}

	/** Arctic draws the crosshair (the game's is hidden). */
	public static boolean ownCrosshair() {
		ClientConfig c = ArcticClient.config();
		return c != null && c.crosshair.enabled;
	}

	/** A chat line arrives: timestamps, mentions, Auto GG. */
	public static Text chat(Text message) {
		ClientConfig c = ArcticClient.config();
		if (c == null) {
			return message;
		}
		String text = message.asUnformattedString();
		ArcticClient.chatLine(text);
		Text out = message;
		if (c.chatMentions && Mentions.mentions(text, ArcticClient.platform().playerName())) {
			out = new LiteralText("§e» §r").append(out);
			ArcticClient.platform().mentionSound();
		}
		if (c.chatTimestamps) {
			String time = new SimpleDateFormat("HH:mm").format(new Date());
			out = new LiteralText("§8[" + time + "] §r").append(out);
		}
		return out;
	}

	/** Around the camera setup: point the camera where Freelook looks. */
	public static void freelookCamera(boolean start) {
		Features f = ArcticClient.features();
		Entity camera = MinecraftClient.getInstance().getCameraEntity();
		if (camera == null) {
			return;
		}
		if (start) {
			if (f == null || !f.freelook()) {
				return;
			}
			savedAngles = new float[] {camera.yaw, camera.pitch, camera.prevYaw, camera.prevPitch};
			camera.yaw = f.lookYaw();
			camera.pitch = f.lookPitch();
			camera.prevYaw = camera.yaw;
			camera.prevPitch = camera.pitch;
		} else if (savedAngles != null) {
			camera.yaw = savedAngles[0];
			camera.pitch = savedAngles[1];
			camera.prevYaw = savedAngles[2];
			camera.prevPitch = savedAngles[3];
			savedAngles = null;
		}
	}

	/** You hit something: its distance, for Reach. */
	public static double attacked(Entity target) {
		return LegacyPlatform.attacked(target);
	}

	/** Streamer mode shows you with the default skin. */
	public static boolean streamerSelf(Object player) {
		return Streamer.on() && player == MinecraftClient.getInstance().player;
	}

	/** The core's drawing, for hooks that draw. */
	public static com.arcticlauncher.client.gfx.Gfx gfx() {
		return new LegacyGfx();
	}

	/** The leave button that asked "click again" (null: none). */
	private static net.minecraft.client.gui.widget.ButtonWidget armedLeave;
	private static String leaveLabel;

	/** True when leaving may go ahead; the first click only asks (when that's on). */
	public static boolean confirmLeave(net.minecraft.client.gui.widget.ButtonWidget button) {
		ClientConfig c = ArcticClient.config();
		if (c == null || !c.confirmLeave || armedLeave == button) {
			return true;
		}
		armedLeave = button;
		leaveLabel = button.message;
		button.message = "Click again to leave";
		return false;
	}

	public static boolean isArmedLeave(Object button) {
		return button != null && button == armedLeave;
	}

	public static void disarmLeave() {
		if (armedLeave != null && leaveLabel != null) {
			armedLeave.message = leaveLabel;
		}
		armedLeave = null;
		leaveLabel = null;
	}

	/** An Arctic look texture once it's uploaded, or null. */
	public static Identifier ready(String hash) {
		return ArcticClient.looks().texture(hash) ? LegacyTextures.look(ArcticClient.looks().frame(hash)) : null;
	}
}
