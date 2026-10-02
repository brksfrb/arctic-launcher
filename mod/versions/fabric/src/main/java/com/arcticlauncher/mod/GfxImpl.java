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
	private static final Identifier ICON = Compat.id(ArcticMod.ID, "icon");
	private static boolean iconLoaded;

	private final GuiGraphicsExtractor g;
	private final Font font;

	public GfxImpl(GuiGraphicsExtractor g) {
		this.g = g;
		this.font = Minecraft.getInstance().font;
	}

	//#if MC < 1.20
	/** Before 1.20: drawing goes through the screen's PoseStack. */
	public static GfxImpl of(com.mojang.blaze3d.vertex.PoseStack pose) {
		return new GfxImpl(new GuiGraphicsExtractor(pose));
	}
	//#endif

	/** The registered texture for a look texture hash. */
	public static Identifier look(String hash) {
		return Compat.id(ArcticMod.ID, "look/" + hash);
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

	private static final java.util.Map<String, Identifier> ASSETS = new java.util.HashMap<String, Identifier>();

	/** A PNG from the jar's {@code assets/arctic/}, registered on first use. */
	private static Identifier asset(String name) {
		Identifier id = ASSETS.get(name);
		if (id != null) {
			return id;
		}
		id = Compat.id(ArcticMod.ID, "asset/" + name);
		ASSETS.put(name, id);
		try (InputStream in = GfxImpl.class.getResourceAsStream("/assets/arctic/" + name + ".png")) {
			NativeImage image = NativeImage.read(in);
			Minecraft.getInstance().getTextureManager().register(id, Compat.texture("Arctic " + name, image));
		} catch (Exception e) {
			ArcticMod.LOG.warn("Arctic asset {}: {}", name, e.toString());
		}
		return id;
	}

	@Override
	public float pixelScale() {
		return (float) Minecraft.getInstance().getWindow().getGuiScale();
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

	//#if MC >= 1.21.6
	@Override
	public void newLayer() {
		g.nextStratum();
	}
	//#endif

	//#if MC >= 26.1
	@Override
	public boolean roundedFill(int x0, int y0, int x1, int y1, int r, int color) {
		Identifier corners = r < 1 ? null : RoundedCorners.texture(r);
		if (corners == null) {
			return false;
		}
		int size = 2 * r;
		g.fill(x0 + r, y0, x1 - r, y1, color);
		if (y1 - r > y0 + r) {
			g.fill(x0, y0 + r, x0 + r, y1 - r, color);
			g.fill(x1 - r, y0 + r, x1, y1 - r, color);
		}
		g.blit(RenderPipelines.GUI_TEXTURED, corners, x0, y0, 0, 0, r, r, r, r, size, size, color);
		g.blit(RenderPipelines.GUI_TEXTURED, corners, x1 - r, y0, r, 0, r, r, r, r, size, size, color);
		g.blit(RenderPipelines.GUI_TEXTURED, corners, x0, y1 - r, 0, r, r, r, r, r, size, size, color);
		g.blit(RenderPipelines.GUI_TEXTURED, corners, x1 - r, y1 - r, r, r, r, r, r, r, size, size, color);
		return true;
	}
	//#endif

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
		long now = System.nanoTime();
		if (widthsFont != font || WIDTHS.size() > MAX_WIDTHS || now - widthsSince > WIDTHS_NANOS) {
			WIDTHS.clear();
			widthsFont = font;
			widthsSince = now;
		}
		Integer width = WIDTHS.get(text);
		if (width == null) {
			width = font.width(text);
			WIDTHS.put(text, width);
		}
		return width;
	}

	/**
	 * Text widths already measured: the HUD measures the same strings every
	 * frame, and the game works each one out glyph by glyph (with bidi
	 * reordering). Render thread only; forgotten now and then, so a reloaded
	 * font's widths come through.
	 */
	private static final java.util.HashMap<String, Integer> WIDTHS = new java.util.HashMap<String, Integer>();
	private static final int MAX_WIDTHS = 1024;
	private static final long WIDTHS_NANOS = 5_000_000_000L;
	private static Object widthsFont;
	private static long widthsSince;

	@Override
	public void texture(String key, int x, int y, int w, int h, float u, float v, int regionW, int regionH, int texW, int texH) {
		Identifier id = key.startsWith("look:") ? look(key.substring(5))
				: key.startsWith("asset:") ? asset(key.substring(6))
				: key.startsWith("dyn:") ? Compat.id(ArcticMod.ID, "dyn/" + key.substring(4)) : icon();
		//#if MC >= 1.21.6
		g.blit(RenderPipelines.GUI_TEXTURED, id, x, y, u, v, w, h, regionW, regionH, texW, texH);
		//#elif MC >= 1.21.2
		g.blit(RenderType::guiTextured, id, x, y, u, v, w, h, regionW, regionH, texW, texH);
		//#else
		// Before 1.21.2 blit has no RenderType lookup: it always draws with the gui texture.
		g.blit(id, x, y, w, h, u, v, regionW, regionH, texW, texH);
		//#endif
	}

	@Override
	public void item(Object stack, int x, int y) {
		net.minecraft.world.item.ItemStack item = (net.minecraft.world.item.ItemStack) stack;
		//#if MC >= 26.1
		g.item(item, x, y);
		g.itemDecorations(font, item, x, y);
		//#else
		g.renderItem(item, x, y);
		g.renderItemDecorations(font, item, x, y);
		//#endif
	}

	@Override
	public void player(int x0, int y0, int x1, int y1, int scale, int mouseX, int mouseY) {
		net.minecraft.client.player.LocalPlayer player = net.minecraft.client.Minecraft.getInstance().player;
		if (player == null) {
			return;
		}
		//#if MC >= 26.1
		net.minecraft.client.gui.screens.inventory.InventoryScreen.extractEntityInInventoryFollowsMouse(g, x0, y0, x1, y1, scale,
				0.0625f, mouseX, mouseY, player);
		//#elif MC >= 1.20.2
		net.minecraft.client.gui.screens.inventory.InventoryScreen.renderEntityInInventoryFollowsMouse(g, x0, y0, x1, y1, scale,
				0.0625f, mouseX, mouseY, player);
		//#else
		// Before 1.20.2 the preview is centered on one point, not a bounding box.
		int cx = (x0 + x1) / 2;
		int cy = (y0 + y1) / 2;
		//#if MC >= 1.20
		net.minecraft.client.gui.screens.inventory.InventoryScreen.renderEntityInInventoryFollowsMouse(g, cx, cy, scale,
				cx - mouseX, cy - mouseY, player);
		//#elif MC >= 1.19.4
		net.minecraft.client.gui.screens.inventory.InventoryScreen.renderEntityInInventoryFollowsMouse(g.pose(), cx, cy, scale,
				cx - mouseX, cy - mouseY, player);
		//#else
		net.minecraft.client.gui.screens.inventory.InventoryScreen.renderEntityInInventory(cx, cy, scale,
				cx - mouseX, cy - mouseY, player);
		//#endif
		//#endif
	}

	@Override
	public void sprite(Object sprite, int x, int y, int w, int h) {
		//#if MC >= 1.21.6
		// 1.21.6+: Gui/Hud.getMobEffectSprite (and friends) hand back an id
		// straight into the GUI sprite atlas.
		g.blitSprite(RenderPipelines.GUI_TEXTURED, (Identifier) sprite, x, y, w, h);
		//#elif MC >= 1.21.2
		g.blitSprite(net.minecraft.client.renderer.RenderType::guiTextured,
				(net.minecraft.client.renderer.texture.TextureAtlasSprite) sprite, x, y, w, h);
		//#else
		// Before 1.21.2 there's no GUI sprite atlas lookup for arbitrary sprites:
		// GameInfo hands back the actual TextureAtlasSprite (see effectSprite()).
		g.blit(x, y, w, h, 0, (net.minecraft.client.renderer.texture.TextureAtlasSprite) sprite);
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
