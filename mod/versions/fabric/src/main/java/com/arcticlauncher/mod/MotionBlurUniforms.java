//#if MC >= 26.1
package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.MotionBlurView;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.mod.mixin.PostChainAccess;
import com.arcticlauncher.mod.mixin.PostPassAccess;
import com.mojang.blaze3d.buffers.Std140Builder;
import com.mojang.blaze3d.systems.RenderSystem;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.Map;
import net.minecraft.client.Camera;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.PostChain;
import net.minecraft.client.renderer.PostPass;
import org.lwjgl.system.MemoryStack;

/**
 * Motion blur on 26.1 and later: the numbers for this frame (see {@link MotionBlurView}) go into the
 * effect's uniform buffer. The game builds that buffer once from the effect's JSON, so every frame a new one
 * replaces it.
 */
public final class MotionBlurUniforms {
	/** The uniform block the effect's shader reads. */
	private static final String BLOCK = "MotionBlurConfig";
	/** GpuBuffer.USAGE_UNIFORM (the type is in another package from 26.3). */
	private static final int USAGE_UNIFORM = 128;
	/** Frames a replaced buffer is kept before it is closed: it may still be in use by a frame in flight. */
	private static final int KEEP = 4;
	/** 4 vec4s: the three columns of the turn, then tan(fov/2) across and up, the smear's length and the stretch. */
	private static final int SIZE = 64;

	private static final MotionBlurView VIEW = new MotionBlurView();
	private static final Deque<Object> RETIRED = new ArrayDeque<Object>();

	private MotionBlurUniforms() {}

	/** Motion blur is off or there is no world: the next frame isn't compared with this one. */
	public static void reset() {
		VIEW.reset();
	}

	/**
	 * Work out this frame's numbers from the camera.
	 *
	 * @return whether the picture turned (false: the blur pass has nothing to do)
	 */
	public static boolean prepare(GameRenderer renderer, Minecraft minecraft) {
		ClientConfig config = ArcticClient.config();
		if (config == null) {
			return false;
		}
		//#if MC >= 26.2
		Camera camera = renderer.mainCamera();
		//#else
		Camera camera = renderer.getMainCamera();
		//#endif
		float aspect = (float) minecraft.getWindow().getWidth() / Math.max(1, minecraft.getWindow().getHeight());
		net.minecraft.world.phys.Vec3 at = camera.position();
		return VIEW.update(camera.yRot(), camera.xRot(), at.x, at.y, at.z, camera.getFov(), aspect, config.motionBlurAmount,
				System.nanoTime(), config.motionBlurMode != 2, config.motionBlurMode != 1);
	}

	/** Put the numbers from {@link #prepare} (or none, when {@code turning} is false) into the effect's buffer. */
	@SuppressWarnings({"rawtypes", "unchecked"})
	public static void write(PostChain chain, boolean turning) {
		for (PostPass pass : ((PostChainAccess) chain).arctic$passes()) {
			Map uniforms = ((PostPassAccess) pass).arctic$customUniforms();
			if (!uniforms.containsKey(BLOCK)) {
				continue;
			}
			try (MemoryStack stack = MemoryStack.stackPush()) {
				float[] m = VIEW.toLast;
				Std140Builder data = Std140Builder.onStack(stack, SIZE)
						.putVec4(m[0], m[1], m[2], 0f)
						.putVec4(m[3], m[4], m[5], 0f)
						.putVec4(m[6], m[7], m[8], 0f)
						.putVec4(VIEW.tanX, VIEW.tanY, turning ? VIEW.scale : 0f, turning ? VIEW.radial : 0f);
				Object buffer = RenderSystem.getDevice().createBuffer(() -> "Arctic motion blur", USAGE_UNIFORM, data.get());
				retire(uniforms.put(BLOCK, buffer));
			}
		}
	}

	private static void retire(Object old) {
		if (old != null) {
			RETIRED.add(old);
		}
		while (RETIRED.size() > KEEP) {
			Object oldest = RETIRED.poll();
			try {
				((AutoCloseable) oldest).close();
			} catch (Exception e) {
				ArcticMod.LOG.warn("motion blur: closing a buffer: {}", e.toString());
			}
		}
	}
}
//#endif
