package com.arcticlauncher.mod;

import net.minecraft.client.Minecraft;
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

	public static Screen optionsScreen(Screen parent) {
		//#if MC >= 26.1 && MC < 26.3
		return new OptionsScreen(parent, mc().options, false);
		//#else
		return new OptionsScreen(parent, mc().options);
		//#endif
	}
}
