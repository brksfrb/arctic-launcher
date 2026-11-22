package com.arcticlauncher.mod;

import com.arcticlauncher.client.feature.TextPixelScale;
import net.minecraft.client.Minecraft;
import org.lwjgl.BufferUtils;
import java.nio.FloatBuffer;
//#if MC >= 1.17
import com.mojang.blaze3d.systems.RenderSystem;
//#endif
//#if MC >= 1.19.3
import org.joml.Matrix4f;
import org.joml.Matrix4fc;
//#elif MC >= 1.15
import com.mojang.math.Matrix4f;
//#endif
//#if MC < 1.17
import org.lwjgl.opengl.GL11;
//#endif

/** Render-thread scratch storage. No matrices, vectors or buffers are allocated per text draw. */
public final class TextDrawScale {
	private static final FloatBuffer MODEL = BufferUtils.createFloatBuffer(16);
	private static final FloatBuffer PROJECTION = BufferUtils.createFloatBuffer(16);
	private static final float[] CLIP = new float[16];
	private static int viewWidth, viewHeight;
	private static boolean clipReady;
	//#if MC >= 1.19.3
	private static final Matrix4f COMBINED = new Matrix4f();
	//#elif MC >= 1.15
	private static final FloatBuffer LOCAL = BufferUtils.createFloatBuffer(16);
	private static final float[] VIEW_MODEL = new float[16];
	//#endif
	// A negative scale defers projection until a badge is encountered when smooth text is off.
	private static float current;
	//#if MC >= 1.19.3
	private static Matrix4fc[] poses = new Matrix4fc[16];
	//#elif MC >= 1.15
	private static Matrix4f[] poses = new Matrix4f[16];
	//#endif
	private static float[] xs = new float[16];
	private static float[] ys = new float[16];
	private static float[] scopes = new float[16];
	private static int depth;
	/** Only the explicit nametagfont self-test changes this to capture the old atlas at the identical camera. */
	static boolean proofBaseline;

	private TextDrawScale() {}

	public static boolean on() {
		//#if MC >= 26.1
		return ArcticPacks.smoothFontCached();
		//#else
		return SmoothFont.on();
		//#endif
	}

	public static int baseScale() {
		//#if MC >= 26.1
		return ArcticPacks.loadedOversample();
		//#else
		return SmoothFont.loadedScale();
		//#endif
	}

	public static float current() {
		return on() && !proofBaseline ? Math.max(0, current) / Math.max(1, baseScale()) : 0;
	}

	public static float framebufferPixels() {
		if (current < 0) {
			int scope = depth - 1;
			current = pixels(
					//#if MC >= 1.15
					poses[scope],
					//#endif
					xs[scope], ys[scope]);
		}
		return current;
	}

	/** Save and restore the framebuffer scale across nested text preparation. */
	public static float set(float pixels) {
		float previous = current;
		current = pixels;
		return previous;
	}

	//#if MC >= 1.19.3
	public static void push(Matrix4fc pose, float x, float y) {
	//#elif MC >= 1.15
	public static void push(Matrix4f pose, float x, float y) {
	//#else
	public static void push(float x, float y) {
	//#endif
		if (depth == scopes.length) {
			int capacity = depth * 2;
			scopes = java.util.Arrays.copyOf(scopes, capacity);
			xs = java.util.Arrays.copyOf(xs, capacity);
			ys = java.util.Arrays.copyOf(ys, capacity);
			//#if MC >= 1.15
			poses = java.util.Arrays.copyOf(poses, capacity);
			//#endif
		}
		//#if MC >= 1.15
		poses[depth] = pose;
		//#endif
		xs[depth] = x;
		ys[depth] = y;
		scopes[depth++] = set(on() ? pixels(
				//#if MC >= 1.15
				pose,
				//#endif
				x, y) : -1);
	}

	public static void pop() {
		current = scopes[--depth];
		//#if MC >= 1.15
		poses[depth] = null;
		//#endif
	}

	public static void restore(float previous) {
		current = previous;
	}

	//#if MC >= 1.19.3
	public static float pixels(Matrix4fc pose, float x, float y) {
	//#elif MC >= 1.15
	public static float pixels(Matrix4f pose, float x, float y) {
	//#else
	public static float pixels(float x, float y) {
	//#endif
		//#if MC >= 1.21.6
		Matrix4fc projection = TextProjection.current();
		if (projection == null) {
			return 0;
		}
		projection.get(PROJECTION);
		//#elif MC >= 1.19.3
		RenderSystem.getProjectionMatrix().get(PROJECTION);
		//#elif MC >= 1.17
		RenderSystem.getProjectionMatrix().store(PROJECTION);
		//#else
		GL11.glGetFloatv(GL11.GL_PROJECTION_MATRIX, PROJECTION);
		//#endif
		//#if MC >= 1.19.3
		//#if MC >= 26.2
		COMBINED.set(RenderSystem.getModelViewStack()).mul(pose).get(MODEL);
		//#else
		COMBINED.set(RenderSystem.getModelViewMatrix()).mul(pose).get(MODEL);
		//#endif
		//#elif MC >= 1.17
		RenderSystem.getModelViewMatrix().store(MODEL);
		pose.store(LOCAL);
		TextPixelScale.multiply(MODEL, LOCAL, VIEW_MODEL);
		for (int i = 0; i < 16; i++) MODEL.put(i, VIEW_MODEL[i]);
		//#elif MC >= 1.15
		GL11.glGetFloatv(GL11.GL_MODELVIEW_MATRIX, MODEL);
		pose.store(LOCAL);
		TextPixelScale.multiply(MODEL, LOCAL, VIEW_MODEL);
		for (int i = 0; i < 16; i++) MODEL.put(i, VIEW_MODEL[i]);
		//#else
		GL11.glGetFloatv(GL11.GL_MODELVIEW_MATRIX, MODEL);
		//#endif
		TextPixelScale.multiply(PROJECTION, MODEL, CLIP);
		viewWidth = Minecraft.getInstance().getWindow().getWidth();
		viewHeight = Minecraft.getInstance().getWindow().getHeight();
		clipReady = true;
		return TextPixelScale.projected(CLIP, x, y, viewWidth, viewHeight);
	}

	/** Snap a badge origin to a whole framebuffer pixel when the projected quad is axis-aligned. */
	public static boolean snapOrigin(float x, float y, float[] out) {
		if (!clipReady) {
			return false;
		}
		return TextPixelScale.snapOrigin(CLIP, x, y, viewWidth, viewHeight, out);
	}
}
