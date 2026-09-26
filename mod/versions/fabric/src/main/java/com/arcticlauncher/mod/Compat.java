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
import net.minecraft.client.gui.screens.options.OptionsScreen;

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
	//#else
	// Before 1.21.6 sprites take a RenderType function instead of a pipeline.
	public static final String BLIT_SPRITE_TINTED = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Ljava/util/function/Function;Lnet/minecraft/resources/Identifier;IIIII)V";
	public static final String BLIT_SPRITE = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Ljava/util/function/Function;Lnet/minecraft/resources/Identifier;IIII)V";
	//#endif

	/** Zoom, Freelook and Fullbright hooks exist for this version. */
	//#if MC >= 26.3
	public static final boolean FEATURES = true;
	//#else
	public static final boolean FEATURES = false;
	//#endif

	private Compat() {}

	// ---- Keys --------------------------------------------------------------------

	/** Is a keyboard key (by Minecraft name) held? */
	public static boolean keyDown(String name) {
		InputConstants.Key key = InputConstants.getKey(name);
		if (key.getType() == InputConstants.Type.MOUSE) {
			return false;
		}
		//#if MC >= 26.3
		return InputConstants.isKeyDown(key.getValue());
		//#elif MC >= 1.21.9
		return InputConstants.isKeyDown(mc().getWindow(), key.getValue());
		//#else
		return InputConstants.isKeyDown(mc().getWindow().getWindow(), key.getValue());
		//#endif
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
		return InputConstants.getKey(name).getDisplayName().getString();
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
		return level.getBiome(pos).unwrapKey()
				//#if MC >= 1.21.11
				.map(key -> key.identifier().getPath())
				//#else
				.map(key -> key.location().getPath())
				//#endif
				.orElse(null);
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
		MouseButtonEvent event = new MouseButtonEvent(x, y, new MouseButtonInfo(InputConstants.MOUSE_BUTTON_LEFT, 0));
		screen.mouseClicked(event, false);
		screen.mouseReleased(event);
		//#else
		screen.mouseClicked(x, y, InputConstants.MOUSE_BUTTON_LEFT);
		screen.mouseReleased(x, y, InputConstants.MOUSE_BUTTON_LEFT);
		//#endif
	}

	public static void joinServer(String serverId) throws Exception {
		User user = mc().getUser();
		//#if MC >= 1.21.9
		mc().services().sessionService().joinServer(user.getProfileId(), user.getAccessToken(), serverId);
		//#else
		mc().getMinecraftSessionService().joinServer(user.getProfileId(), user.getAccessToken(), serverId);
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
}
