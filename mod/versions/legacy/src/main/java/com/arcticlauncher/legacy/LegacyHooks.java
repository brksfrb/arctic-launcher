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

import com.arcticlauncher.client.Lang;
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

	/** The held-item layer of players and mobs is drawing an item (see OldHeldLayerMixin). */
	public static volatile boolean heldLayerActive;

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

	/** What the visible chunks were last worked out for: {yaw, pitch, fov}. */
	private static float[] culledFor = new float[3];

	/**
	 * The game only works out which chunks are in view again when the player's
	 * own angles or position change. Freelook turns the camera (not the
	 * player) and Zoom narrows the view, so without this the chunks behind you
	 * or outside the zoomed view stay missing until you move.
	 */
	public static void refreshVisibleChunks() {
		Features f = ArcticClient.features();
		MinecraftClient mc = MinecraftClient.getInstance();
		if (f == null || mc.worldRenderer == null) {
			return;
		}
		boolean free = f.freelook();
		float[] now = {free ? f.lookYaw() : 0f, free ? f.lookPitch() : 0f, worldFov};
		if (now[0] != culledFor[0] || now[1] != culledFor[1] || now[2] != culledFor[2]) {
			culledFor = now;
			mc.worldRenderer.scheduleTerrainUpdate();
		}
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

	private static boolean chunkBordersOurs;

	/** Chunk Borders: the game's F3+G (1.10 and newer), switched on while the switch is on. */
	private static void syncChunkBorders() {
		//#if MC >= 1.10
		if (ArcticClient.config() == null) {
			return;
		}
		boolean want = ArcticClient.config().chunkBorders;
		if (!want && !chunkBordersOurs) {
			return;
		}
		net.minecraft.client.render.debug.DebugRenderer debug = MinecraftClient.getInstance().debugRenderer;
		if (debug == null) {
			return;
		}
		// The game only offers "toggle" (it returns the new state): toggling twice reads the state and changes nothing.
		boolean shown = !debug.toggleChunkBorders();
		debug.toggleChunkBorders();
		if (want && !shown) {
			debug.toggleChunkBorders();
			chunkBordersOurs = true;
		} else if (!want && chunkBordersOurs) {
			if (shown) {
				debug.toggleChunkBorders();
			}
			chunkBordersOurs = false;
		}
		//#endif
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

	private static final LegacyWindowSize WINDOW_SIZE = new LegacyWindowSize();

	/**
	 * The game's size follows its window. LWJGL 2 tells the game once ("resized"), and a window made
	 * bigger while the game was still loading (the launcher maximizes it as it opens) can use that
	 * up: the game then keeps drawing a small picture in the corner of a big black window.
	 * Also report changes vanilla already synchronized, so cursor/key recovery still runs.
	 */
	private static boolean syncWindowSize() {
		MinecraftClient mc = MinecraftClient.getInstance();
		boolean created = org.lwjgl.opengl.Display.isCreated();
		return WINDOW_SIZE.sync(created,
				created ? org.lwjgl.opengl.Display.getWidth() : 0,
				created ? org.lwjgl.opengl.Display.getHeight() : 0,
				created && org.lwjgl.opengl.Display.isFullscreen(), mc.width, mc.height,
				(w, h) -> ((com.arcticlauncher.legacy.mixin.MinecraftClientResizeInvoker) mc).arctic$resize(w, h));
	}

	private static int cursorLogged;

	// Yarn calls unpressAll "releaseAllKeys" on every legacy target (1.8.9 through 1.12.2).
	// It clears held states and queued presses for all bindings, including movement/attack/use.
	private static final LegacyCursorGuard CURSOR_GUARD = new LegacyCursorGuard(
			net.minecraft.client.option.KeyBinding::releaseAllKeys);
	private static final LegacyCursorGuard.Mouse CURSOR_MOUSE = new LegacyCursorGuard.Mouse() {
		@Override
		public boolean isGrabbed() {
			return org.lwjgl.input.Mouse.isGrabbed();
		}

		@Override
		public void setGrabbed(boolean grabbed) {
			org.lwjgl.input.Mouse.setGrabbed(grabbed);
		}

		@Override
		public void clearNativeCursor() throws org.lwjgl.LWJGLException {
			org.lwjgl.input.Mouse.setNativeCursor(null);
		}
	};

	/**
	 * Always request the default, ungrabbed pointer for screens. A real unclip also runs on
	 * window creation, resize, fullscreen switches and screen entry, even when LWJGL reports
	 * no grab. Both paths release stuck keys. Keeping the force event-driven leaves ordinary
	 * gameplay keys alone and avoids toggling/flickering on every menu tick.
	 */
	private static void cursorGuard(boolean screenOpen, boolean resized) {
		try {
			boolean forced = CURSOR_GUARD.update(CURSOR_MOUSE,
					org.lwjgl.opengl.Display.isCreated() && org.lwjgl.input.Mouse.isCreated(),
					screenOpen, org.lwjgl.opengl.Display.isFullscreen(), resized);
			if (forced && cursorLogged < 5) {
				cursorLogged++;
				ArcticLegacy.LOG.info("Arctic: reset the LWJGL 2 mouse clip (screen open: {}, window resized: {})", screenOpen, resized);
			}
		} catch (Exception e) {
			// Window/mouse recreation can briefly reject these requests; retry on the next tick.
		}
	}

	private static boolean titled;

	/** The window's title names Arctic, once (LWJGL 2 sets it before any mod runs). */
	private static void titleWindow() {
		titled = true;
		if (System.getProperty("arctic.launcher") != null) {
			org.lwjgl.opengl.Display.setTitle("Minecraft " + ArcticClient.platform().minecraftVersion() + " - Arctic");
		}
	}

	public static void tick(boolean screenOpen) {
		if (!titled) {
			titleWindow();
		}
		boolean resized = syncWindowSize();
		ArcticLegacy.PLATFORM.tick();
		syncHitboxes();
		syncChunkBorders();
		com.arcticlauncher.legacy.replay.LegacyReplayRecording.tick();
		ArcticClient.tick(screenOpen);
		// Features can reassert bindings (e.g. toggled sneak); release after those updates.
		// Keep ticking in-world as well, so window events recover keys even without chat.
		cursorGuard(screenOpen || MinecraftClient.getInstance().currentScreen != null, resized);
	}

	/** Over the game, after Minecraft's own HUD. */
	public static void renderHud() {
		LegacyGfx g = new LegacyGfx();
		ArcticClient.renderHud(g);
		if ((ownCrosshair() || ArcticClient.config().rangeIndicator) && MinecraftClient.getInstance().options.perspective == 0) {
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

	private static net.minecraft.inventory.slot.Slot sortFocus;

	/** Client Sort: the slot under the mouse when a key goes down in a chest or inventory screen. */
	public static void sortKey(net.minecraft.inventory.slot.Slot focused) {
		sortFocus = focused;
		ArcticClient.containerSort();
	}

	/** The slot the mouse was over at the last key press (what Client Sort sorts). */
	public static net.minecraft.inventory.slot.Slot sortFocus() {
		return sortFocus;
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

	/** Shulker Peek: what the box under the cursor holds. */
	public static void drawPeek(net.minecraft.inventory.slot.Slot slot, int mouseX, int mouseY) {
		if (slot == null || !slot.hasStack()) {
			return;
		}
		com.arcticlauncher.client.feature.ContainerPeek.hover(new LegacyGfx(), mouseX, mouseY, slot.getStack());
	}

	/**
	 * The Arctic badge of the chosen style at ({@code x}, {@code y}), the top left of a text line: always
	 * {@code Badge.SIZE} square from the line's top (the font's glyphs are 8 high from there), as the
	 * newer versions' font picture, with the text's own transparency and, with a shadow, its shadow.
	 */
	public static void drawBadge(float x, float y, int argb, boolean shadow, int style, boolean tab) {
		net.minecraft.util.Identifier texture = LegacyTextures.resolve("asset:" + com.arcticlauncher.client.feature.Badge.name(style) + (tab ? "_tab" : "_tag"));
		if (texture == null) {
			return;
		}
		int side = com.arcticlauncher.client.feature.Badge.size(tab);
		int resolution = side * 8;
		y -= tab ? 1 : 0;
		int alpha = argb >>> 24;
		// Text with no alpha is opaque, as the game draws it.
		float a = alpha == 0 ? 1f : alpha / 255f;
		MinecraftClient.getInstance().getTextureManager().bindTexture(texture);
		com.mojang.blaze3d.platform.GlStateManager.enableBlend();
		com.mojang.blaze3d.platform.GlStateManager.blendFuncSeparate(org.lwjgl.opengl.GL11.GL_SRC_ALPHA,
				org.lwjgl.opengl.GL11.GL_ONE_MINUS_SRC_ALPHA, org.lwjgl.opengl.GL11.GL_ONE, org.lwjgl.opengl.GL11.GL_ZERO);
		// The player list's picture has no shadow, its names do.
		if (shadow && !tab) {
			com.mojang.blaze3d.platform.GlStateManager.color(0.25f, 0.25f, 0.25f, a);
			net.minecraft.client.gui.DrawableHelper.drawTexture(Math.round(x) + 1, Math.round(y) + 1, 0f, 0f, resolution, resolution, side, side, resolution, resolution);
		}
		com.mojang.blaze3d.platform.GlStateManager.color(1f, 1f, 1f, a);
		net.minecraft.client.gui.DrawableHelper.drawTexture(Math.round(x), Math.round(y), 0f, 0f, resolution, resolution, side, side, resolution, resolution);
	}

	/** RGBA chat glyph, using the same one-texture path as the badge. */
	public static void drawEmote(com.arcticlauncher.client.feature.EmoteAtlas.Emote emote, float x, float y, float alpha, boolean shadow) {
		net.minecraft.util.Identifier texture = LegacyTextures.resolve("asset:emotes");
		if (texture == null) return;
		MinecraftClient.getInstance().getTextureManager().bindTexture(texture);
		com.mojang.blaze3d.platform.GlStateManager.enableBlend();
		com.mojang.blaze3d.platform.GlStateManager.blendFuncSeparate(org.lwjgl.opengl.GL11.GL_SRC_ALPHA,
				org.lwjgl.opengl.GL11.GL_ONE_MINUS_SRC_ALPHA, org.lwjgl.opengl.GL11.GL_ONE, org.lwjgl.opengl.GL11.GL_ZERO);
		float tint = shadow ? 0.25f : 1f;
		com.mojang.blaze3d.platform.GlStateManager.color(tint, tint, tint, alpha);
		net.minecraft.client.gui.DrawableHelper.drawTexture(Math.round(x), Math.round(y) - 1, emote.u(), emote.v(), 144, 144, 10, 10, 1152, 1152);
	}

	/** The ping number in the player list, in the place of the ping bars; false when the latency is unknown. */
	public static boolean drawTabPing(int width, int x, int y, int ms) {
		return com.arcticlauncher.client.ui.TabPing.draw(new LegacyGfx(), width, x, y, ms);
	}

	/** Pop-ups on the screens that cover the game; in the game the HUD shows them. */
	public static void drawScreenNotices(net.minecraft.client.gui.screen.Screen screen) {
		ClientConfig c = ArcticClient.config();
		if (c == null || ArcticClient.platform() == null || screen instanceof LegacyPageScreen) {
			return;
		}
		String name = screen.getClass().getSimpleName().toLowerCase(java.util.Locale.ROOT);
		boolean covers = name.contains("terrain") || name.contains("loading") || name.contains("progress");
		if (!ArcticClient.platform().inWorld() || covers) {
			com.arcticlauncher.client.gfx.Draw.fancy = c.fancy;
			com.arcticlauncher.client.notice.Notices.render(new LegacyGfx(), ArcticClient.style());
		}
	}

	/** The emoji button of the chat screen (and its box). */
	public static void drawEmoji(int mouseX, int mouseY, int width, int height) {
		ClientConfig c = ArcticClient.config();
		if (c != null) {
			com.arcticlauncher.client.feature.EmojiPicker.render(new LegacyGfx(), c, mouseX, mouseY, width, height);
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
	public static Text chat(Text message, String source) {
		ClientConfig c = ArcticClient.config();
		if (c == null) {
			return message;
		}
		String text = message.asUnformattedString();
		ArcticClient.chatLine(text);
		Text out = LegacyChatCopy.tag(message, source, false);
		if (c.chatMentions && Mentions.mentions(text, ArcticClient.platform().playerName())) {
			out = LegacyChatCopy.tag(new LiteralText("§e» §r"), source, false).append(out);
			ArcticClient.platform().mentionSound();
		}
		return stamped(out, source, new java.util.Date());
	}

	/** A line restored from the saved chat: no sound or stacking, but the time it arrived (0: no stamp). */
	public static Text restoredChat(Text message, long time) {
		ClientConfig c = ArcticClient.config();
		if (c == null || time <= 0) {
			return message;
		}
		String source = message.asUnformattedString();
		return stamped(LegacyChatCopy.tag(message, source, false), source, new java.util.Date(time));
	}

	private static Text stamped(Text out, String source, java.util.Date when) {
		ClientConfig c = ArcticClient.config();
		if (c.chatTimestamps) {
			String shown = com.arcticlauncher.client.feature.ChatStamp.text(c, when);
			net.minecraft.client.font.TextRenderer font = MinecraftClient.getInstance().textRenderer;
			// The slot padding sits beside the tagged stamp, so the hover/click target is only the time.
			Text pad = new LiteralText("§8" + com.arcticlauncher.client.feature.ChatStampWidth.padding(c, font, font::getStringWidth, shown));
			Text stamp = LegacyChatCopy.tag(new LiteralText("§8" + (c.chatStampBold ? "§l" : "") + (c.chatStampItalic ? "§o" : "")
					+ shown + "§r"), source, true);
			out = com.arcticlauncher.client.feature.ChatStampWidth.right()
					? new LiteralText("").append(pad).append(stamp).append(out)
					: new LiteralText("").append(stamp).append(pad).append(out);
		} else if (c.chatCopy) {
			out = new LiteralText("").append(LegacyChatCopy.tag(new LiteralText("§8[Copy] §r"), source, true)).append(out);
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
		button.message = Lang.t("platform.click_again_to_leave");
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
