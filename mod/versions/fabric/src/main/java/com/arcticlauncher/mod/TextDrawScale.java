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
	//#if MC >= 1.19.3
	private static final Matrix4f COMBINED = new Matrix4f();
	//#elif MC >= 1.15
	private static final FloatBuffer LOCAL = BufferUtils.createFloatBuffer(16);
	private static final float[] VIEW_MODEL = new float[16];
	//#endif
	private static float current;
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
		return current;
	}

	/** Save/restore allows nested draw/preparation calls without leaking a world's scale into GUI text. */
	public static float set(float pixels) {
		float previous = current;
		current = on() ? pixels / Math.max(1, baseScale()) : 0;
		return previous;
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
		if (!on() || proofBaseline) {
			return 0;
		}
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
		return TextPixelScale.projected(CLIP, x, y, Minecraft.getInstance().getWindow().getWidth(),
				Minecraft.getInstance().getWindow().getHeight());
	}
}
