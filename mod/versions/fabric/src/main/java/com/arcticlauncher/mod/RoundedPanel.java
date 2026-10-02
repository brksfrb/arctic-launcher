//#if MC >= 26.1
package com.arcticlauncher.mod;

//#if MC >= 26.3
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
//#else
import com.mojang.blaze3d.pipeline.RenderPipeline;
//#endif
import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.navigation.ScreenRectangle;
import net.minecraft.client.gui.render.TextureSetup;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.state.gui.GuiElementRenderState;
import net.minecraft.client.renderer.texture.AbstractTexture;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;
import org.joml.Matrix3x2f;
import org.joml.Matrix3x2fc;
import org.jspecify.annotations.Nullable;

/**
 * A rounded panel as one GUI piece: nine quads (four anti-aliased corners,
 * four edges, the middle) over one small texture, instead of three fills
 * and four corner images. The game compares every GUI piece with the ones
 * under it to stack them, so a HUD of panels costs a fraction of before.
 */
final class RoundedPanel implements GuiElementRenderState {
	/** Larger corners fall back to the pieces. */
	private static final int MAX_RADIUS = 64;
	private static final int SAMPLES = 4;
	private static final Map<Integer, Identifier> TEXTURES = new HashMap<>();
	private static final MethodHandle SCISSOR_STACK;
	private static final MethodHandle PEEK;

	static {
		MethodHandle stack = null;
		MethodHandle peek = null;
		try {
			java.lang.reflect.Field field = GuiGraphicsExtractor.class.getDeclaredField("scissorStack");
			field.setAccessible(true);
			java.lang.reflect.Method method = field.getType().getDeclaredMethod("peek");
			method.setAccessible(true);
			stack = MethodHandles.lookup().unreflectGetter(field);
			peek = MethodHandles.lookup().unreflect(method);
		} catch (ReflectiveOperationException | RuntimeException e) {
			// No scissor to read: panels are drawn as pieces instead.
		}
		SCISSOR_STACK = stack;
		PEEK = peek;
	}

	private final TextureSetup texture;
	private final Matrix3x2f pose;
	private final int x0;
	private final int y0;
	private final int x1;
	private final int y1;
	private final int r;
	private final int color;
	private final @Nullable ScreenRectangle scissor;
	private final @Nullable ScreenRectangle bounds;

	private RoundedPanel(TextureSetup texture, Matrix3x2f pose, int x0, int y0, int x1, int y1, int r, int color,
			@Nullable ScreenRectangle scissor) {
		this.texture = texture;
		this.pose = pose;
		this.x0 = x0;
		this.y0 = y0;
		this.x1 = x1;
		this.y1 = y1;
		this.r = r;
		this.color = color;
		this.scissor = scissor;
		ScreenRectangle area = new ScreenRectangle(x0, y0, x1 - x0, y1 - y0).transformMaxBounds(pose);
		this.bounds = scissor != null ? scissor.intersection(area) : area;
	}

	/** Hand the game a rounded panel as one piece; false if it can't be (then it's drawn in pieces). */
	static boolean add(GuiGraphicsExtractor g, int x0, int y0, int x1, int y1, int r, int color) {
		if (r < 1 || r > MAX_RADIUS || x1 - x0 < 2 * r || y1 - y0 < 2 * r || PEEK == null) {
			return false;
		}
		ScreenRectangle scissor;
		try {
			scissor = (ScreenRectangle) PEEK.invoke(SCISSOR_STACK.invoke(g));
		} catch (Throwable e) {
			return false;
		}
		AbstractTexture texture = Minecraft.getInstance().getTextureManager().getTexture(texture(r));
		TextureSetup setup = TextureSetup.singleTexture(texture.getTextureView(), texture.getSampler());
		RoundedPanel panel = new RoundedPanel(setup, new Matrix3x2f(g.pose()), x0, y0, x1, y1, r, color, scissor);
		((com.arcticlauncher.mod.mixin.GuiGraphicsStateAccess) g).arctic$guiRenderState().addGuiElement(panel);
		return true;
	}

	@Override
	public void buildVertices(VertexConsumer out) {
		// The texture is (2r + 2) wide: corner quarters, with a 2-texel opaque cross between them.
		float size = 2 * r + 2;
		float c0 = r / size;
		float m0 = (r + 0.5f) / size;
		float m1 = (r + 1.5f) / size;
		float c1 = (r + 2) / size;
		int xa = x0 + r;
		int xb = x1 - r;
		int ya = y0 + r;
		int yb = y1 - r;
		quad(out, x0, y0, xa, ya, 0, 0, c0, c0);
		quad(out, xa, y0, xb, ya, m0, 0, m1, c0);
		quad(out, xb, y0, x1, ya, c1, 0, 1, c0);
		quad(out, x0, ya, xa, yb, 0, m0, c0, m1);
		quad(out, xa, ya, xb, yb, m0, m0, m1, m1);
		quad(out, xb, ya, x1, yb, c1, m0, 1, m1);
		quad(out, x0, yb, xa, y1, 0, c1, c0, 1);
		quad(out, xa, yb, xb, y1, m0, c1, m1, 1);
		quad(out, xb, yb, x1, y1, c1, c1, 1, 1);
	}

	private void quad(VertexConsumer out, float qx0, float qy0, float qx1, float qy1, float u0, float v0, float u1, float v1) {
		if (qx1 <= qx0 || qy1 <= qy0) {
			return;
		}
		out.addVertexWith2DPose(pose, qx0, qy0).setUv(u0, v0).setColor(color);
		out.addVertexWith2DPose(pose, qx0, qy1).setUv(u0, v1).setColor(color);
		out.addVertexWith2DPose(pose, qx1, qy1).setUv(u1, v1).setColor(color);
		out.addVertexWith2DPose(pose, qx1, qy0).setUv(u1, v0).setColor(color);
	}

	@Override
	public RenderPipeline pipeline() {
		return RenderPipelines.GUI_TEXTURED;
	}

	@Override
	public TextureSetup textureSetup() {
		return texture;
	}

	@Override
	public @Nullable ScreenRectangle scissorArea() {
		return scissor;
	}

	@Override
	public @Nullable ScreenRectangle bounds() {
		return bounds;
	}

	/** Corner quarters of radius r around a 2-texel opaque cross, made on first use. */
	private static Identifier texture(int r) {
		Identifier id = TEXTURES.get(r);
		if (id == null) {
			id = Compat.id(ArcticMod.ID, "dyn/panel_" + r);
			DynamicTexture texture = Compat.texture("Arctic rounded panel", image(r));
			Minecraft.getInstance().getTextureManager().register(id, texture);
			TEXTURES.put(r, id);
		}
		return id;
	}

	private static NativeImage image(int r) {
		int size = 2 * r + 2;
		NativeImage image = new NativeImage(size, size, false);
		for (int y = 0; y < size; y++) {
			for (int x = 0; x < size; x++) {
				// Texel → the circle's texel (the cross is the circle's widest row and column, opaque).
				boolean crossX = x == r || x == r + 1;
				boolean crossY = y == r || y == r + 1;
				float alpha;
				if (crossX || crossY) {
					alpha = 1f;
				} else {
					int cx = x < r ? x : x - 2;
					int cy = y < r ? y : y - 2;
					alpha = coverage(cx, cy, r);
				}
				image.setPixel(x, y, Math.round(255 * alpha) << 24 | 0xFFFFFF);
			}
		}
		return image;
	}

	/** How much of pixel (x, y) lies inside the circle around (r, r); as RoundedCorners. */
	private static float coverage(int x, int y, int r) {
		int inside = 0;
		for (int sy = 0; sy < SAMPLES; sy++) {
			for (int sx = 0; sx < SAMPLES; sx++) {
				float dx = x + (sx + 0.5f) / SAMPLES - r;
				float dy = y + (sy + 0.5f) / SAMPLES - r;
				if (dx * dx + dy * dy <= (float) r * r) {
					inside++;
				}
			}
		}
		return inside / (float) (SAMPLES * SAMPLES);
	}
}
//#endif
