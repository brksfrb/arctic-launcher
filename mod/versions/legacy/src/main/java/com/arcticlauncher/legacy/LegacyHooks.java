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

	private static boolean hitboxesOurs;

	/** The Hitboxes mod: the game's F3+B flag, set while the switch is on (and only unset if this set it). */
	private static void syncHitboxes() {
		net.minecraft.client.render.entity.EntityRenderDispatcher dispatcher = MinecraftClient.getInstance().getEntityRenderManager();
		if (dispatcher == null || ArcticClient.config() == null) {
			return;
		}
		boolean want = ArcticClient.config().hitboxes;
		if (want && !dispatcher.getRenderHitboxes()) {
			dispatcher.setRenderHitboxes(true);
			hitboxesOurs = true;
		} else if (!want && hitboxesOurs) {
			dispatcher.setRenderHitboxes(false);
			hitboxesOurs = false;
		}
	}

	/**
	 * The game's size follows its window. LWJGL 2 tells the game once ("resized"), and a window made
	 * bigger while the game was still loading (the launcher maximizes it as it opens) can use that
	 * up: the game then keeps drawing a small picture in the corner of a big black window.
	 */
	private static void syncWindowSize() {
		MinecraftClient mc = MinecraftClient.getInstance();
		int w = org.lwjgl.opengl.Display.getWidth();
		int h = org.lwjgl.opengl.Display.getHeight();
		if (w > 0 && h > 0 && !org.lwjgl.opengl.Display.isFullscreen() && (w != mc.width || h != mc.height)) {
			((com.arcticlauncher.legacy.mixin.MinecraftClientResizeInvoker) mc).arctic$resize(w, h);
		}
	}

	public static void tick(boolean screenOpen) {
		ArcticLegacy.PLATFORM.tick();
		syncWindowSize();
		syncHitboxes();
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

	// ---- Lock Slots ------------------------------------------------------------------

	/** The number of {@code slot} in the player's inventory (0-8 hotbar … 40 off hand), or -1 in another container. */
	private static int inventorySlot(net.minecraft.inventory.slot.Slot slot) {
		return slot != null && slot.inventory instanceof net.minecraft.entity.player.PlayerInventory
				? ((com.arcticlauncher.legacy.mixin.SlotInvAccess) slot).arctic$invSlot() : -1;
	}

	/** A click in an inventory screen on {@code slot}; {@code swapWith}: the hotbar slot of a number-key swap, else -1. True drops it. */
	public static boolean slotClick(net.minecraft.inventory.slot.Slot slot, int swapWith) {
		ClientConfig c = ArcticClient.config();
		return c != null && ArcticClient.platform() != null
				&& com.arcticlauncher.client.feature.SlotLocks.click(c, ArcticClient.platform(), inventorySlot(slot), swapWith);
	}

	/** A padlock on the slot when it is locked (drawn with the slot, so under tooltips). */
	public static void drawPadlock(net.minecraft.inventory.slot.Slot slot) {
		ClientConfig c = ArcticClient.config();
		if (c == null) {
			return;
		}
		int index = c.slotLocks ? inventorySlot(slot) : -1;
		if (index >= 0 && com.arcticlauncher.client.feature.SlotLocks.locked(c, index)) {
			com.arcticlauncher.client.feature.SlotLocks.draw(new LegacyGfx(), slot.x, slot.y);
		}
		if (c.containerSearch && com.arcticlauncher.client.feature.InventorySearch.filtering()) {
			com.arcticlauncher.client.feature.InventorySearch.slot(new LegacyGfx(), slot.x, slot.y,
					slot.hasStack() ? slot.getStack().getCustomName() : null);
		}
	}

	/** Inventory Search: a key (LWJGL 2 code) and its character in a chest or inventory screen; true when it was used. */
	public static boolean containerKey(char typed, int key) {
		int core = key == 1 ? com.arcticlauncher.client.Keys.ESCAPE : key == 28 ? com.arcticlauncher.client.Keys.ENTER
				: key == 14 ? com.arcticlauncher.client.Keys.BACKSPACE : key == 33 ? com.arcticlauncher.client.Keys.F : -1;
		boolean used = core >= 0 && ArcticClient.containerKey(core, 1, net.minecraft.client.gui.screen.Screen.hasControlDown());
		return (typed >= 32 && ArcticClient.containerChar(typed)) || used;
	}

	/** Inventory Search: its line over a chest or inventory screen (and the note that one is open). */
	public static void drawSearch() {
		ClientConfig c = ArcticClient.config();
		com.arcticlauncher.client.feature.InventorySearch.seen();
		if (c != null) {
			com.arcticlauncher.client.feature.InventorySearch.render(new LegacyGfx(), c);
		}
	}

	/** The drop key is pressed while the slot in hand is locked. */
	public static boolean dropLocked() {
		ClientConfig c = ArcticClient.config();
		net.minecraft.entity.player.ClientPlayerEntity p = MinecraftClient.getInstance().player;
		return c != null && p != null && com.arcticlauncher.client.feature.SlotLocks.drop(c, p.inventory.selectedSlot);
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

	/**
	 * A player's skin texture. Legacy Yarn names it {@code getCapeId} (its {@code getSkinId} is the cape):
	 * the player renderer draws the body with {@code getCapeId}, the cape layer with {@code getSkinId}.
	 */
	public static Identifier skinOf(net.minecraft.client.network.AbstractClientPlayerEntity player) {
		return player.getCapeId();
	}

	/** An Arctic look texture once it's uploaded, or null. */
	public static Identifier ready(String hash) {
		// Only once uploaded: before that Minecraft looks for it in the resource packs and fails the draw.
		return ArcticClient.looks().texture(hash) ? LegacyTextures.resolve("look:" + ArcticClient.looks().frame(hash)) : null;
	}
}
