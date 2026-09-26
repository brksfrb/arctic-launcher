package com.arcticlauncher.mod;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.Minecraft;
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
	//#else
	public static final String BLIT_SPRITE_TINTED = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lcom/mojang/blaze3d/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIIII)V";
	public static final String BLIT_SPRITE = "Lnet/minecraft/client/gui/GuiGraphicsExtractor;blitSprite(Lcom/mojang/blaze3d/pipeline/RenderPipeline;Lnet/minecraft/resources/Identifier;IIII)V";
	//#endif

	private Compat() {}

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
