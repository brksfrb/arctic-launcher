package com.arcticlauncher.mod;

import com.mojang.blaze3d.platform.InputConstants;
import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.client.User;
import net.minecraft.world.entity.player.Player;
//#if MC >= 1.21.9
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.client.input.MouseButtonInfo;
//#endif
import net.minecraft.client.gui.screens.Screen;
//#if MC >= 1.21
import net.minecraft.client.gui.screens.options.OptionsScreen;
//#else
import net.minecraft.client.gui.screens.OptionsScreen;
//#endif
//#if MC >= 1.21.6
//#if MC >= 26.2
import net.minecraft.client.gui.Hud;
//#else
import net.minecraft.client.gui.Gui;
//#endif
import net.minecraft.core.Holder;
import net.minecraft.world.effect.MobEffect;
//#endif

/**
 * The places where Minecraft versions differ, so the rest of the adapter
 * reads the same everywhere. Mixin descriptors that differ live here too.
 */
public final class Compat {
	/** GuiGraphics.blitSprite(pipeline, sprite, x, y, w, h, color). */
	//#if MC >= 26.3
	public static final String BLIT_SPRITE_TINTED = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lcom/mojang/renderpearl/api/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIIII)V";
	public static final String BLIT_SPRITE = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lcom/mojang/renderpearl/api/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIII)V";
	//#elif MC >= 1.21.6
	public static final String BLIT_SPRITE_TINTED = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lcom/mojang/blaze3d/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIIII)V";
	public static final String BLIT_SPRITE = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lcom/mojang/blaze3d/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIII)V";
	//#elif MC >= 1.21.2
	// 1.21.2 - 1.21.5 sprites take a RenderType function instead of a pipeline.
	public static final String BLIT_SPRITE_TINTED = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Ljava/util/function/Function;Lnet/minecraft/resources/Identifier;IIIII)V";
	public static final String BLIT_SPRITE = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Ljava/util/function/Function;Lnet/minecraft/resources/Identifier;IIII)V";
	//#else
	// Before 1.21.2 blitSprite has no RenderType lookup at all.
	public static final String BLIT_SPRITE_TINTED = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lnet/minecraft/resources/Identifier;IIIII)V";
	public static final String BLIT_SPRITE = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lnet/minecraft/resources/Identifier;IIII)V";
	//#endif

	/** Zoom, Freelook, Fullbright and the rest of the Features/View/Crosshair tabs: every version. */
	public static final boolean FEATURES = true;

	private Compat() {}

	/** Plain text (Component.literal from 1.19; TextComponent before). */
	public static net.minecraft.network.chat.MutableComponent literal(String text) {
		//#if MC >= 1.19
		return net.minecraft.network.chat.Component.literal(text);
		//#else
		return new net.minecraft.network.chat.TextComponent(text);
		//#endif
	}

	/** Translated text. */
	public static net.minecraft.network.chat.MutableComponent translatable(String key, Object... args) {
		//#if MC >= 1.19
		return net.minecraft.network.chat.Component.translatable(key, args);
		//#else
		return new net.minecraft.network.chat.TranslatableComponent(key, args);
		//#endif
	}

	/** No text. */
	public static net.minecraft.network.chat.MutableComponent empty() {
		//#if MC >= 1.19
		return net.minecraft.network.chat.Component.empty();
		//#else
		return new net.minecraft.network.chat.TextComponent("");
		//#endif
	}

	/** The translation key of a translated text, or null. */
	public static String translationKey(net.minecraft.network.chat.Component text) {
		//#if MC >= 1.19
		return text.getContents() instanceof net.minecraft.network.chat.contents.TranslatableContents
				? ((net.minecraft.network.chat.contents.TranslatableContents) text.getContents()).getKey() : null;
		//#else
		return text instanceof net.minecraft.network.chat.TranslatableComponent
				? ((net.minecraft.network.chat.TranslatableComponent) text).getKey() : null;
		//#endif
	}

	/** A widget's left edge (a method from 1.19.3, a field before). */
	public static int widgetX(net.minecraft.client.gui.components.AbstractWidget w) {
		//#if MC >= 1.19.3
		return w.getX();
		//#else
		return w.x;
		//#endif
	}

	public static int widgetHeight(net.minecraft.client.gui.components.AbstractWidget w) {
		//#if MC >= 1.16
		return w.getHeight();
		//#else
		// No getter before 1.16; vanilla buttons are 20 high.
		return 20;
		//#endif
	}

	/** Whether text can show Arctic's badge glyph (it needs fonts in text, 1.16+). */
	//#if MC >= 1.16
	public static final boolean BADGES = true;
	//#else
	public static final boolean BADGES = false;
	//#endif

	public static int widgetY(net.minecraft.client.gui.components.AbstractWidget w) {
		//#if MC >= 1.19.3
		return w.getY();
		//#else
		return w.y;
		//#endif
	}

	/** Frames drawn in the last second. */
	public static int fps() {
		//#if MC >= 1.19.4
		return Minecraft.getInstance().getFps();
		//#else
		String fps = Minecraft.getInstance().fpsString;
		int end = fps.indexOf(' ');
		try {
			return Integer.parseInt(end > 0 ? fps.substring(0, end) : fps);
		} catch (NumberFormatException e) {
			return 0;
		}
		//#endif
	}

	/** Send chat, or a command when it starts with "/". */
	public static void sendChat(String text) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.player == null || text == null || text.isEmpty()) {
			return;
		}
		//#if MC >= 1.19.3
		if (text.startsWith("/")) {
			mc.player.connection.sendCommand(text.substring(1));
		} else {
			mc.player.connection.sendChat(text);
		}
		//#elif MC >= 1.19
		if (text.startsWith("/")) {
			mc.player.commandUnsigned(text.substring(1));
		} else {
			mc.player.chatSigned(text, null);
		}
		//#else
		mc.player.chat(text);
		//#endif
	}

	// ---- Keys --------------------------------------------------------------------

	/** Is a keyboard key (by Minecraft name) held? */
	public static boolean keyDown(String name) {
		InputConstants.Key key = InputConstants.getKey(name);
		// Unbound ("key.keyboard.unknown" is -1): asking GLFW about it logs a GL error every frame.
		if (key.getValue() < 0) {
			return false;
		}
		if (key.getType() == InputConstants.Type.MOUSE) {
			int button = key.getValue();
			return button >= 0 && button < MOUSE_BUTTONS.length && MOUSE_BUTTONS[button];
		}
		//#if MC >= 26.3
		return InputConstants.isKeyDown(key.getValue());
		//#elif MC >= 1.21.9
		return InputConstants.isKeyDown(mc().getWindow(), key.getValue());
		//#else
		return InputConstants.isKeyDown(mc().getWindow().getWindow(), key.getValue());
		//#endif
	}

	/** The mouse in GUI coordinates. */
	public static double[] guiMouse() {
		Minecraft mc = mc();
		com.mojang.blaze3d.platform.Window w = mc.getWindow();
		return new double[] {
				mc.mouseHandler.xpos() * w.getGuiScaledWidth() / Math.max(1, w.getScreenWidth()),
				mc.mouseHandler.ypos() * w.getGuiScaledHeight() / Math.max(1, w.getScreenHeight()),
		};
	}

	/** Mouse buttons held right now (kept by MouseHandlerMixin; some versions can't be asked). */
	private static final boolean[] MOUSE_BUTTONS = new boolean[8];

	public static void mouseButton(int button, boolean down) {
		if (button >= 0 && button < MOUSE_BUTTONS.length) {
			MOUSE_BUTTONS[button] = down;
		}
	}

	/** The Minecraft name of a keyboard key code, like "key.keyboard.c". */
	public static String keyName(int code) {
		//#if MC >= 26.3
		return InputConstants.Type.KEYBOARD.getOrCreate(code).getName();
		//#else
		return InputConstants.Type.KEYSYM.getOrCreate(code).getName();
		//#endif
	}

	public static String keyLabel(String name) {
		//#if MC >= 1.16
		return InputConstants.getKey(name).getDisplayName().getString();
		//#else
		// Keys named by translation ("key.keyboard.f9"), else by the keyboard layout's letter.
		InputConstants.Key key = InputConstants.getKey(name);
		String label = net.minecraft.client.resources.language.I18n.get(key.getName());
		if (!label.equals(key.getName())) {
			return label;
		}
		String letter = org.lwjgl.glfw.GLFW.glfwGetKeyName(key.getValue(), 0);
		return letter != null ? letter.toUpperCase(java.util.Locale.ROOT) : key.getName();
		//#endif
	}

	// ---- World -------------------------------------------------------------------

	/** World time in ticks (26.1 moved day time to world clocks; game time is close). */
	public static long dayTime(net.minecraft.world.level.Level level) {
		//#if MC >= 26.1
		return level.getLevelData().getGameTime();
		//#else
		return level.getDayTime();
		//#endif
	}

	/** The biome's id path, like "snowy_plains". */
	public static String biomeId(net.minecraft.world.level.Level level, net.minecraft.core.BlockPos pos) {
		//#if MC >= 1.18.2
		return level.getBiome(pos).unwrapKey()
				//#if MC >= 1.21.11
				.map(key -> key.identifier().getPath())
				//#else
				.map(key -> key.location().getPath())
				//#endif
				.orElse(null);
		//#elif MC >= 1.16.2
		net.minecraft.resources.ResourceLocation id = level.registryAccess()
				.registryOrThrow(net.minecraft.core.Registry.BIOME_REGISTRY).getKey(level.getBiome(pos));
		return id == null ? null : id.getPath();
		//#else
		net.minecraft.resources.ResourceLocation id = net.minecraft.core.Registry.BIOME.getKey(level.getBiome(pos));
		return id == null ? null : id.getPath();
		//#endif
	}

	private static Minecraft mc() {
		return Minecraft.getInstance();
	}

	public static Screen screen() {
		//#if MC >= 26.2
		return mc().gui.screen();
		//#else
		return mc().screen;
		//#endif
	}

	/** The world's field of view last frame (before 26.1, where Camera doesn't keep it). */
	public static volatile double worldFov = 70;

	public static void setScreen(Screen screen) {
		//#if MC >= 26.2
		mc().gui.setScreen(screen);
		//#else
		mc().setScreen(screen);
		//#endif
	}

	/** F1 is on (the vanilla HUD is hidden). */
	public static boolean hudHidden() {
		//#if MC >= 26.2
		return mc().gui.hud.isHidden();
		//#else
		return mc().options.hideGui;
		//#endif
	}

	/** F3 is on (the debug overlay is showing). */
	public static boolean debugScreenShowing() {
		//#if MC >= 1.20.2
		return mc().getDebugOverlay().showDebugScreen();
		//#else
		// Before 1.20.2 there's no queryable overlay object, just the option flag.
		return mc().options.renderDebug;
		//#endif
	}

	//#if MC >= 1.21.6
	/** The mob effect's icon in the GUI sprite atlas (26.2 split Hud out of Gui). */
	public static net.minecraft.resources.Identifier mobEffectSprite(Holder<MobEffect> effect) {
		//#if MC >= 26.2
		return Hud.getMobEffectSprite(effect);
		//#else
		return Gui.getMobEffectSprite(effect);
		//#endif
	}
	//#endif

	/** A GPU texture for an image (named for debugging where supported). */
	public static DynamicTexture texture(String name, NativeImage image) {
		//#if MC >= 1.21.5
		return new DynamicTexture(() -> name, image);
		//#else
		return new DynamicTexture(image);
		//#endif
	}

	/** A left click on a screen, without going through the input handler. */
	public static void click(Screen screen, double x, double y) {
		//#if MC >= 1.21.9
		MouseButtonEvent event = new MouseButtonEvent(x, y, new MouseButtonInfo(com.arcticlauncher.mod.compat.KeyCodes.MOUSE_LEFT, 0));
		screen.mouseClicked(event, false);
		screen.mouseReleased(event);
		//#else
		screen.mouseClicked(x, y, com.arcticlauncher.mod.compat.KeyCodes.MOUSE_LEFT);
		screen.mouseReleased(x, y, com.arcticlauncher.mod.compat.KeyCodes.MOUSE_LEFT);
		//#endif
	}

	public static void joinServer(String serverId) throws Exception {
		User user = mc().getUser();
		//#if MC >= 1.21.9
		mc().services().sessionService().joinServer(user.getProfileId(), user.getAccessToken(), serverId);
		//#elif MC >= 1.20.2
		mc().getMinecraftSessionService().joinServer(user.getProfileId(), user.getAccessToken(), serverId);
		//#else
		// Before 1.20.2 authlib's joinServer took a full GameProfile, not just the id.
		mc().getMinecraftSessionService().joinServer(user.getGameProfile(), user.getAccessToken(), serverId);
		//#endif
	}

	public static String playerName(Player player) {
		//#if MC >= 1.21.9
		return player.getPlainTextName();
		//#else
		return player.getScoreboardName();
		//#endif
	}

	public static Screen optionsScreen(Screen parent) {
		//#if MC >= 26.1 && MC < 26.3
		return new OptionsScreen(parent, mc().options, false);
		//#else
		return new OptionsScreen(parent, mc().options);
		//#endif
	}

	/** A player's UUID (authlib 7 made GameProfile a record). */
	public static java.util.UUID profileId(com.mojang.authlib.GameProfile profile) {
		//#if MC >= 1.21.9
		return profile.id();
		//#else
		return profile.getId();
		//#endif
	}

	/** A namespaced id (1.21 replaced the public constructor with this factory). */
	public static net.minecraft.resources.Identifier id(String namespace, String path) {
		//#if MC >= 1.21
		return net.minecraft.resources.Identifier.fromNamespaceAndPath(namespace, path);
		//#else
		return new net.minecraft.resources.Identifier(namespace, path);
		//#endif
	}

	/** The Arctic snowflake as a text glyph (font {@code arctic:badge}). */
	public static net.minecraft.network.chat.MutableComponent arcticBadge() {
		//#if MC < 1.16
		// No custom fonts in text before 1.16: no badge.
		return literal("");
		//#else
		net.minecraft.resources.Identifier font = id(ArcticMod.ID, "badge");
		net.minecraft.network.chat.MutableComponent badge = literal("");
		//#if MC >= 1.21.9
		return badge.withStyle(style -> style.withFont(new net.minecraft.network.chat.FontDescription.Resource(font)));
		//#else
		return badge.withStyle(style -> style.withFont(font));
		//#endif
		//#endif
	}

	/**
	 * Close the world's connection the way the pause menu does before
	 * leaving: a singleplayer server only stops once its player has quit, and
	 * leaving waits for it to stop. (From 1.21.9 leaving does this itself.)
	 */
	public static void quitLevel() {
		Minecraft mc = mc();
		if (mc.level == null) {
			return;
		}
		//#if MC >= 1.21.9
		//#elif MC >= 1.21.6
		mc.level.disconnect(net.minecraft.client.multiplayer.ClientLevel.DEFAULT_QUIT_MESSAGE);
		//#else
		mc.level.disconnect();
		//#endif
	}

	// ---- Entities (fields before 1.17) ----------------------------------------------

	public static net.minecraft.core.BlockPos blockPos(net.minecraft.world.entity.Entity e) {
		//#if MC >= 1.16
		return e.blockPosition();
		//#else
		return new net.minecraft.core.BlockPos(e);
		//#endif
	}

	/** The camera: 0 first person, 1 third person behind, 2 third person in front. */
	public static int camera() {
		//#if MC >= 1.16
		return mc().options.getCameraType().ordinal();
		//#else
		return mc().options.thirdPersonView;
		//#endif
	}

	public static void setCamera(int view) {
		//#if MC >= 1.16
		mc().options.setCameraType(net.minecraft.client.CameraType.values()[view]);
		//#else
		mc().options.thirdPersonView = view;
		//#endif
	}

	// ---- Button labels (plain strings before 1.16) -------------------------------------

	/** The button label's translation key (null if none; before 1.16 matched against {@code candidates}). */
	public static String labelKey(net.minecraft.client.gui.components.AbstractWidget w, java.util.Collection<String> candidates) {
		//#if MC >= 1.16
		return translationKey(w.getMessage());
		//#else
		for (String key : candidates) {
			if (net.minecraft.client.resources.language.I18n.get(key).equals(w.getMessage())) {
				return key;
			}
		}
		return null;
		//#endif
	}

	public static Object label(net.minecraft.client.gui.components.AbstractWidget w) {
		return w.getMessage();
	}

	public static String labelText(net.minecraft.client.gui.components.AbstractWidget w) {
		//#if MC >= 1.16
		return w.getMessage().getString();
		//#else
		return w.getMessage();
		//#endif
	}

	public static void setLabel(net.minecraft.client.gui.components.AbstractWidget w, Object label) {
		//#if MC >= 1.16
		w.setMessage((net.minecraft.network.chat.Component) label);
		//#else
		w.setMessage(label instanceof net.minecraft.network.chat.Component
				? ((net.minecraft.network.chat.Component) label).getColoredString() : (String) label);
		//#endif
	}

	public static float yRot(net.minecraft.world.entity.Entity e) {
		//#if MC >= 1.17
		return e.getYRot();
		//#else
		return e.yRot;
		//#endif
	}

	public static float xRot(net.minecraft.world.entity.Entity e) {
		//#if MC >= 1.17
		return e.getXRot();
		//#else
		return e.xRot;
		//#endif
	}

	public static void setYRot(net.minecraft.world.entity.Entity e, float yaw) {
		//#if MC >= 1.17
		e.setYRot(yaw);
		//#else
		e.yRot = yaw;
		//#endif
	}

	public static void setXRot(net.minecraft.world.entity.Entity e, float pitch) {
		//#if MC >= 1.17
		e.setXRot(pitch);
		//#else
		e.xRot = pitch;
		//#endif
	}

	public static boolean removed(net.minecraft.world.entity.Entity e) {
		//#if MC >= 1.17
		return e.isRemoved();
		//#else
		return e.removed;
		//#endif
	}

	/** Where the entity was last frame is where it is now (no smoothing from an old spot). */
	public static void settle(net.minecraft.world.entity.Entity e) {
		//#if MC >= 1.17
		e.setOldPosAndRot();
		//#else
		e.xo = e.getX();
		e.yo = e.getY();
		e.zo = e.getZ();
		e.xOld = e.getX();
		e.yOld = e.getY();
		e.zOld = e.getZ();
		e.yRotO = e.yRot;
		e.xRotO = e.xRot;
		//#endif
	}

	public static net.minecraft.world.entity.player.Inventory inventory(net.minecraft.world.entity.player.Player p) {
		//#if MC >= 1.17
		return p.getInventory();
		//#else
		return p.inventory;
		//#endif
	}

	/** A green speaker (for players talking in voice chat). */
	public static net.minecraft.network.chat.MutableComponent speakerBadge() {
		//#if MC >= 1.16
		net.minecraft.network.chat.MutableComponent speaker = arcticBadge();
		speaker = literal("").withStyle(speaker.getStyle());
		return speaker.withStyle(style -> style.withColor(net.minecraft.network.chat.TextColor.fromRgb(SPEAKING_COLOR)));
		//#else
		return literal("").withStyle(net.minecraft.ChatFormatting.GREEN);
		//#endif
	}

	private static final int SPEAKING_COLOR = 0x86EFAC;
}
