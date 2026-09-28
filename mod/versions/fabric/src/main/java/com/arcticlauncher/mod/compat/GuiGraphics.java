//#if MC < 1.20
package com.arcticlauncher.mod.compat;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.PoseStack;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiComponent;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.world.item.ItemStack;

/**
 * Before 1.20 the GUI drew through a PoseStack and static helpers; this
 * stands in for 1.20's GuiGraphics (the part Arctic uses), so the rest of
 * the adapter stays the same. The build points GuiGraphics here on these
 * versions.
 */
public final class GuiGraphics {
	private final PoseStack pose;
	private final Minecraft mc = Minecraft.getInstance();

	public GuiGraphics(PoseStack pose) {
		this.pose = pose;
	}

	public PoseStack pose() {
		return pose;
	}

	public int guiWidth() {
		return mc.getWindow().getGuiScaledWidth();
	}

	public int guiHeight() {
		return mc.getWindow().getGuiScaledHeight();
	}

	public void fill(int x0, int y0, int x1, int y1, int color) {
		//#if MC >= 1.16
		GuiComponent.fill(pose, x0, y0, x1, y1, color);
		//#else
		GuiComponent.fill(pose.last().pose(), x0, y0, x1, y1, color);
		//#endif
	}

	public void fillGradient(int x0, int y0, int x1, int y1, int top, int bottom) {
		Gradient.INSTANCE.draw(pose, x0, y0, x1, y1, top, bottom);
	}

	/** GuiComponent keeps its gradient to subclasses. */
	private static final class Gradient extends GuiComponent {
		static final Gradient INSTANCE = new Gradient();

		void draw(PoseStack pose, int x0, int y0, int x1, int y1, int top, int bottom) {
			//#if MC >= 1.16
			fillGradient(pose, x0, y0, x1, y1, top, bottom);
			//#else
			RenderSystem.pushMatrix();
			RenderSystem.multMatrix(pose.last().pose());
			fillGradient(x0, y0, x1, y1, top, bottom);
			RenderSystem.popMatrix();
			//#endif
		}
	}

	public int drawString(Font font, String text, int x, int y, int color, boolean shadow) {
		RenderSystem.enableBlend();
		//#if MC >= 1.16
		return shadow ? font.drawShadow(pose, text, x, y, color) : font.draw(pose, text, x, y, color);
		//#else
		int[] end = new int[1];
		withPose(() -> end[0] = shadow ? font.drawShadow(text, x, y, color) : font.draw(text, x, y, color));
		return end[0];
		//#endif
	}

	public void blit(ResourceLocation texture, int x, int y, int w, int h, float u, float v, int regionW, int regionH, int texW, int texH) {
		bind(texture);
		RenderSystem.enableBlend();
		RenderSystem.defaultBlendFunc();
		//#if MC >= 1.16
		GuiComponent.blit(pose, x, y, w, h, u, v, regionW, regionH, texW, texH);
		//#else
		withPose(() -> GuiComponent.blit(x, y, w, h, u, v, regionW, regionH, texW, texH));
		//#endif
	}

	/** A sprite from an atlas (like an effect icon). */
	public void blit(int x, int y, int w, int h, int z, TextureAtlasSprite sprite) {
		//#if MC >= 1.19.3
		bind(sprite.atlasLocation());
		//#else
		bind(sprite.atlas().location());
		//#endif
		RenderSystem.enableBlend();
		//#if MC >= 1.16
		GuiComponent.blit(pose, x, y, z, w, h, sprite);
		//#else
		withPose(() -> GuiComponent.blit(x, y, z, w, h, sprite));
		//#endif
	}

	public void renderItem(ItemStack stack, int x, int y) {
		//#if MC >= 1.19.4
		mc.getItemRenderer().renderGuiItem(pose, stack, x, y);
		//#else
		withPose(() -> mc.getItemRenderer().renderGuiItem(stack, x, y));
		//#endif
	}

	public void renderItemDecorations(Font font, ItemStack stack, int x, int y) {
		//#if MC >= 1.19.4
		mc.getItemRenderer().renderGuiItemDecorations(pose, font, stack, x, y);
		//#else
		withPose(() -> mc.getItemRenderer().renderGuiItemDecorations(font, stack, x, y));
		//#endif
	}

	private void bind(ResourceLocation texture) {
		//#if MC >= 1.17
		RenderSystem.setShaderTexture(0, texture);
		//#else
		mc.getTextureManager().bind(texture);
		//#endif
	}

	//#if MC < 1.19.4
	/**
	 * Items (and before 1.16 everything) drew with the global model-view
	 * matrix before 1.19.4: apply this pose to it.
	 */
	private void withPose(Runnable draw) {
		//#if MC < 1.17
		RenderSystem.pushMatrix();
		RenderSystem.multMatrix(pose.last().pose());
		draw.run();
		RenderSystem.popMatrix();
		//#else
		PoseStack view = RenderSystem.getModelViewStack();
		view.pushPose();
		view.mulPoseMatrix(pose.last().pose());
		RenderSystem.applyModelViewMatrix();
		draw.run();
		view.popPose();
		RenderSystem.applyModelViewMatrix();
		//#endif
	}
	//#endif

	public void enableScissor(int x0, int y0, int x1, int y1) {
		double scale = mc.getWindow().getGuiScale();
		int height = mc.getWindow().getHeight();
		int x = (int) (x0 * scale);
		int y = (int) (height - y1 * scale);
		int w = (int) ((x1 - x0) * scale);
		int h = (int) ((y1 - y0) * scale);
		//#if MC >= 1.16
		RenderSystem.enableScissor(x, y, w, h);
		//#else
		org.lwjgl.opengl.GL11.glEnable(org.lwjgl.opengl.GL11.GL_SCISSOR_TEST);
		org.lwjgl.opengl.GL11.glScissor(x, y, w, h);
		//#endif
	}

	public void disableScissor() {
		//#if MC >= 1.16
		RenderSystem.disableScissor();
		//#else
		org.lwjgl.opengl.GL11.glDisable(org.lwjgl.opengl.GL11.GL_SCISSOR_TEST);
		//#endif
	}
}
//#endif
