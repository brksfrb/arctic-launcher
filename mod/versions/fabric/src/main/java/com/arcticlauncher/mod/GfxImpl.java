package com.arcticlauncher.mod;

import com.arcticlauncher.client.gfx.Gfx;
import com.mojang.blaze3d.platform.NativeImage;
import java.io.InputStream;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
//#if MC >= 1.21.6
import net.minecraft.client.renderer.RenderPipelines;
//#else
import net.minecraft.client.renderer.RenderType;
//#endif
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;

/** The core's drawing primitives on 26.3's GUI renderer. */
public final class GfxImpl implements Gfx {
	private static final Identifier ICON = Identifier.fromNamespaceAndPath(ArcticMod.ID, "icon");
	private static boolean iconLoaded;

	private final GuiGraphicsExtractor g;
	private final Font font;

	public GfxImpl(GuiGraphicsExtractor g) {
		this.g = g;
		this.font = Minecraft.getInstance().font;
	}

	/** The registered texture for a look texture hash. */
	public static Identifier look(String hash) {
		return Identifier.fromNamespaceAndPath(ArcticMod.ID, "look/" + hash);
	}

	/**
	 * Without Fabric API, mod assets aren't game resources, so the icon is
	 * read from the jar and registered on first use (render thread).
	 */
	private static Identifier icon() {
		if (!iconLoaded) {
			iconLoaded = true;
			try (InputStream in = GfxImpl.class.getResourceAsStream("/assets/arctic/icon.png")) {
				NativeImage image = NativeImage.read(in);
				Minecraft.getInstance().getTextureManager().register(ICON, Compat.texture("Arctic icon", image));
			} catch (Exception e) {
				ArcticMod.LOG.warn("Arctic icon: {}", e.toString());
			}
		}
		return ICON;
	}

	@Override
	public int width() {
		return g.guiWidth();
	}

	@Override
	public int height() {
		return g.guiHeight();
	}

	@Override
	public void fill(int x0, int y0, int x1, int y1, int color) {
		g.fill(x0, y0, x1, y1, color);
	}

	@Override
	public void gradient(int x0, int y0, int x1, int y1, int top, int bottom) {
		g.fillGradient(x0, y0, x1, y1, top, bottom);
	}

	@Override
	public void text(String text, int x, int y, int color, boolean shadow) {
		//#if MC >= 26.1
		g.text(font, text, x, y, color, shadow);
		//#else
		g.drawString(font, text, x, y, color, shadow);
		//#endif
	}

	@Override
	public int textWidth(String text) {
		return font.width(text);
	}

	@Override
	public void texture(String key, int x, int y, int w, int h, float u, float v, int regionW, int regionH, int texW, int texH) {
		Identifier id = key.startsWith("look:") ? look(key.substring(5)) : icon();
		//#if MC >= 1.21.6
		g.blit(RenderPipelines.GUI_TEXTURED, id, x, y, u, v, w, h, regionW, regionH, texW, texH);
		//#else
		g.blit(RenderType::guiTextured, id, x, y, u, v, w, h, regionW, regionH, texW, texH);
		//#endif
	}

	@Override
	public void item(Object stack, int x, int y) {
		//#if MC >= 26.1
		net.minecraft.world.item.ItemStack item = (net.minecraft.world.item.ItemStack) stack;
		g.item(item, x, y);
		g.itemDecorations(font, item, x, y);
		//#endif
	}

	@Override
	public void sprite(Object sprite, int x, int y, int w, int h) {
		//#if MC >= 26.1
		g.blitSprite(RenderPipelines.GUI_TEXTURED, (Identifier) sprite, x, y, w, h);
		//#endif
	}

	@Override
	public void push() {
		//#if MC >= 1.21.6
		g.pose().pushMatrix();
		//#else
		g.pose().pushPose();
		//#endif
	}

	@Override
	public void pop() {
		//#if MC >= 1.21.6
		g.pose().popMatrix();
		//#else
		g.pose().popPose();
		//#endif
	}

	@Override
	public void translate(float x, float y) {
		//#if MC >= 1.21.6
		g.pose().translate(x, y);
		//#else
		g.pose().translate(x, y, 0);
		//#endif
	}

	@Override
	public void scale(float s) {
		//#if MC >= 1.21.6
		g.pose().scale(s, s);
		//#else
		g.pose().scale(s, s, 1);
		//#endif
	}

	@Override
	public void scissor(int x0, int y0, int x1, int y1) {
		g.enableScissor(x0, y0, x1, y1);
	}

	@Override
	public void endScissor() {
		g.disableScissor();
	}
}
