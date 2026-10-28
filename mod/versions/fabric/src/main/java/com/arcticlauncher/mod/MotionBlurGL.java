package com.arcticlauncher.mod;

//#if MC < 26.1
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.MotionBlurView;
import com.arcticlauncher.client.config.ClientConfig;
import com.mojang.blaze3d.pipeline.RenderTarget;
import java.nio.IntBuffer;
import java.nio.FloatBuffer;
//#if MC >= 1.14
import net.minecraft.client.Camera;
//#else
import net.minecraft.world.entity.Entity;
//#endif
import net.minecraft.client.Minecraft;
import org.lwjgl.BufferUtils;
import org.lwjgl.opengl.GL;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL12;
import org.lwjgl.opengl.GL13;
import org.lwjgl.opengl.GL14;
import org.lwjgl.opengl.GL20;
import org.lwjgl.opengl.GL30;
import org.lwjgl.opengl.GL33;

/**
 * Motion blur before 26.1 (26.1+ uses a post effect in the client's pack): right after the world is
 * drawn, a copy of the frame is smeared along the way the picture moved since the last frame (see
 * {@link MotionBlurView}) and drawn back. Done with plain OpenGL on textures and framebuffers of its own,
 * so it doesn't depend on Minecraft's renderer (which changes nearly every version); every bit of GL
 * state it touches is put back, so Minecraft's state tracking stays right.
 */
public final class MotionBlurGL {
	/** GL_BLEND_COLOR (not among LWJGL's GL14 constants). */
	private static final int GL_BLEND_COLOR = 0x8005;

	private static int width;
	private static int height;
	/** A copy of this frame, read from while the smeared one is drawn. */
	private static int prevTexture;
	private static int prevFramebuffer;
	/** A framebuffer on the game's own frame texture (made for whichever texture is current). */
	private static int mainFramebuffer;
	private static int mainTexture;
	private static final MotionBlurView VIEW = new MotionBlurView();
	private static int program;
	private static int sceneUniform;
	private static int toLastUniform;
	private static int tanUniform;
	private static int scaleUniform;
	private static int radialUniform;
	private static int vertexArray;
	private static boolean broken;

	private static final IntBuffer INTS = BufferUtils.createIntBuffer(16);
	private static final FloatBuffer FLOATS = BufferUtils.createFloatBuffer(16);

	/** VulkanMod draws the game with Vulkan: OpenGL calls would do nothing (or harm). */
	static final boolean VULKAN_MOD = VulkanCompat.ACTIVE;
	/**
	 * Whether sampler objects (OpenGL 3.3) are there to unbind. Asked on first use, not when the class
	 * loads: under VulkanMod there is no OpenGL to ask.
	 */
	private static final class Samplers {
		static final boolean SUPPORTED = GL.getCapabilities().OpenGL33;
	}
	private MotionBlurGL() {}

	/** After the world is drawn, before the HUD. */
	public static void apply() {
		ClientConfig c = ArcticClient.config();
		Minecraft mc = Minecraft.getInstance();
		if (c == null || !c.motionBlur || mc.level == null || broken || VULKAN_MOD || VulkanCompat.postBlur()) {
			VIEW.reset();
			return;
		}
		if (!GL.getCapabilities().OpenGL30) {
			broken = true;
			ArcticMod.LOG.warn("motion blur needs OpenGL 3.0 framebuffers");
			return;
		}
		RenderTarget target = mc.getMainRenderTarget();
		int texture = colorTexture(target);
		if (texture <= 0) {
			return;
		}
		float aspect = (float) target.width / Math.max(1, target.height);
		//#if MC >= 1.21.11
		Camera camera = mc.gameRenderer.getMainCamera();
		float yaw = camera.yRot();
		float pitch = camera.xRot();
		net.minecraft.world.phys.Vec3 at = camera.position();
		double x = at.x;
		double y = at.y;
		double z = at.z;
		//#elif MC >= 1.14
		Camera camera = mc.gameRenderer.getMainCamera();
		float yaw = camera.getYRot();
		float pitch = camera.getXRot();
		net.minecraft.world.phys.Vec3 at = camera.getPosition();
		double x = at.x;
		double y = at.y;
		double z = at.z;
		//#else
		Entity view = mc.getCameraEntity();
		if (view == null) {
			return;
		}
		float partial = mc.getFrameTime();
		float yaw = view.yRotO + (view.yRot - view.yRotO) * partial;
		float pitch = view.xRotO + (view.xRot - view.xRotO) * partial;
		double x = view.xOld + (view.x - view.xOld) * partial;
		double y = view.yOld + (view.y - view.yOld) * partial;
		double z = view.zOld + (view.z - view.zOld) * partial;
		//#endif
		if (!VIEW.update(yaw, pitch, x, y, z, fovDegrees(mc), aspect, c.motionBlurAmount, System.nanoTime(),
				c.motionBlurMode != 2, c.motionBlurMode != 1)) {
			return;
		}
		Saved saved = Saved.read();
		try {
			prepare(target.width, target.height, texture);
			// A copy of the frame to read from, while the smeared picture is drawn over the frame itself.
			GL30.glBindFramebuffer(GL30.GL_READ_FRAMEBUFFER, mainFramebuffer);
			GL30.glBindFramebuffer(GL30.GL_DRAW_FRAMEBUFFER, prevFramebuffer);
			GL30.glBlitFramebuffer(0, 0, width, height, 0, 0, width, height, GL11.GL_COLOR_BUFFER_BIT, GL11.GL_NEAREST);
			smear();
		} catch (RuntimeException e) {
			broken = true;
			ArcticMod.LOG.warn("motion blur: {}", e.toString());
		} finally {
			saved.restore();
		}
	}

	/** The field of view the game is set to, in degrees (it can't see the sprint or zoom change on top). */
	private static float fovDegrees(Minecraft mc) {
		//#if MC >= 1.19
		return mc.options.fov().get();
		//#else
		return (float) mc.options.fov;
		//#endif
	}

	private static int colorTexture(RenderTarget target) {
		//#if MC >= 1.21.5
		com.mojang.blaze3d.textures.GpuTexture t = target.getColorTexture();
		return t instanceof com.mojang.blaze3d.opengl.GlTexture ? ((com.mojang.blaze3d.opengl.GlTexture) t).glId() : -1;
		//#elif MC >= 1.16
		return target.getColorTextureId();
		//#else
		return target.colorTextureId;
		//#endif
	}

	/** Our textures sized to the frame, and a framebuffer on the frame's texture. */
	private static void prepare(int w, int h, int texture) {
		if (w != width || h != height || prevTexture == 0) {
			if (prevTexture != 0) {
				GL11.glDeleteTextures(prevTexture);
			}
			prevTexture = GL11.glGenTextures();
			GL11.glBindTexture(GL11.GL_TEXTURE_2D, prevTexture);
			GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_LINEAR);
			GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_LINEAR);
			GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_S, GL12.GL_CLAMP_TO_EDGE);
			GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_T, GL12.GL_CLAMP_TO_EDGE);
			GL11.glTexImage2D(GL11.GL_TEXTURE_2D, 0, GL11.GL_RGBA8, w, h, 0, GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE, (java.nio.ByteBuffer) null);
			if (prevFramebuffer == 0) {
				prevFramebuffer = GL30.glGenFramebuffers();
			}
			GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, prevFramebuffer);
			GL30.glFramebufferTexture2D(GL30.GL_FRAMEBUFFER, GL30.GL_COLOR_ATTACHMENT0, GL11.GL_TEXTURE_2D, prevTexture, 0);
			width = w;
			height = h;
		}
		if (mainFramebuffer == 0) {
			mainFramebuffer = GL30.glGenFramebuffers();
		}
		if (texture != mainTexture) {
			GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, mainFramebuffer);
			GL30.glFramebufferTexture2D(GL30.GL_FRAMEBUFFER, GL30.GL_COLOR_ATTACHMENT0, GL11.GL_TEXTURE_2D, texture, 0);
			mainTexture = texture;
		}
	}

	/** The copy of the frame, smeared along the movement and drawn over the frame. */
	private static void smear() {
		GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, mainFramebuffer);
		GL11.glViewport(0, 0, width, height);
		GL11.glDisable(GL11.GL_DEPTH_TEST);
		GL11.glDisable(GL11.GL_CULL_FACE);
		GL11.glDisable(GL11.GL_SCISSOR_TEST);
		GL11.glDepthMask(false);
		// The colour is replaced as it is; the frame's alpha stays (it isn't dependable).
		GL11.glEnable(GL11.GL_BLEND);
		GL14.glBlendFuncSeparate(GL11.GL_ONE, GL11.GL_ZERO, GL11.GL_ZERO, GL11.GL_ONE);
		GL13.glActiveTexture(GL13.GL_TEXTURE0);
		GL11.glBindTexture(GL11.GL_TEXTURE_2D, prevTexture);
		// The game may leave a sampler object on unit 0 (1.21.x does): its mipmap filtering makes our
		// texture, which has none, read as black.
		if (Samplers.SUPPORTED) {
			GL33.glBindSampler(0, 0);
		}
		if (program == 0) {
			program = makeProgram();
			sceneUniform = GL20.glGetUniformLocation(program, "Scene");
			toLastUniform = GL20.glGetUniformLocation(program, "ToLast");
			tanUniform = GL20.glGetUniformLocation(program, "Tan");
			scaleUniform = GL20.glGetUniformLocation(program, "Scale");
			radialUniform = GL20.glGetUniformLocation(program, "Radial");
			//#if MC >= 1.17
			vertexArray = GL30.glGenVertexArrays();
			//#endif
		}
		GL20.glUseProgram(program);
		GL20.glUniform1i(sceneUniform, 0);
		FLOATS.clear();
		FLOATS.put(VIEW.toLast).flip();
		GL20.glUniformMatrix3fv(toLastUniform, false, FLOATS);
		GL20.glUniform2f(tanUniform, VIEW.tanX, VIEW.tanY);
		GL20.glUniform1f(scaleUniform, VIEW.scale);
		GL20.glUniform1f(radialUniform, VIEW.radial);
		//#if MC >= 1.17
		// 1.17+ runs a core OpenGL 3.2 context: a fullscreen triangle.
		GL30.glBindVertexArray(vertexArray);
		GL11.glDrawArrays(GL11.GL_TRIANGLES, 0, 3);
		//#else
		// Before 1.17 the context is a compatibility one: a plain textured quad.
		GL11.glMatrixMode(GL11.GL_PROJECTION);
		GL11.glPushMatrix();
		GL11.glLoadIdentity();
		GL11.glMatrixMode(GL11.GL_MODELVIEW);
		GL11.glPushMatrix();
		GL11.glLoadIdentity();
		boolean tex = GL11.glIsEnabled(GL11.GL_TEXTURE_2D);
		boolean alphaTest = GL11.glIsEnabled(GL11.GL_ALPHA_TEST);
		boolean lighting = GL11.glIsEnabled(GL11.GL_LIGHTING);
		boolean fog = GL11.glIsEnabled(GL11.GL_FOG);
		GL11.glEnable(GL11.GL_TEXTURE_2D);
		GL11.glDisable(GL11.GL_ALPHA_TEST);
		GL11.glDisable(GL11.GL_LIGHTING);
		GL11.glDisable(GL11.GL_FOG);
		GL11.glGetFloatv(GL11.GL_CURRENT_COLOR, FLOATS);
		float r = FLOATS.get(0), g = FLOATS.get(1), b = FLOATS.get(2), a = FLOATS.get(3);
		GL11.glColor4f(1f, 1f, 1f, 1f);
		GL11.glBegin(GL11.GL_QUADS);
		GL11.glTexCoord2f(0f, 0f);
		GL11.glVertex2f(-1f, -1f);
		GL11.glTexCoord2f(1f, 0f);
		GL11.glVertex2f(1f, -1f);
		GL11.glTexCoord2f(1f, 1f);
		GL11.glVertex2f(1f, 1f);
		GL11.glTexCoord2f(0f, 1f);
		GL11.glVertex2f(-1f, 1f);
		GL11.glEnd();
		GL11.glColor4f(r, g, b, a);
		setEnabled(GL11.GL_TEXTURE_2D, tex);
		setEnabled(GL11.GL_ALPHA_TEST, alphaTest);
		setEnabled(GL11.GL_LIGHTING, lighting);
		setEnabled(GL11.GL_FOG, fog);
		GL11.glMatrixMode(GL11.GL_PROJECTION);
		GL11.glPopMatrix();
		GL11.glMatrixMode(GL11.GL_MODELVIEW);
		GL11.glPopMatrix();
		//#endif
	}

	/**
	 * The smear: each pixel is the average of sixteen samples along the line it moved on since the last frame,
	 * found by turning the pixel's view direction back by the camera's turn (see MotionBlurView). A little
	 * noise per pixel moves the sixteen samples so they don't show as steps.
	 */
	private static final String SMEAR = "uniform sampler2D Scene;\n"
			+ "uniform mat3 ToLast;\n"
			+ "uniform vec2 Tan;\n"
			+ "uniform float Scale;\n"
			+ "uniform float Radial;\n"
			+ "const int SAMPLES = 16;\n"
			+ "const float LONGEST = 0.25;\n"
			+ "vec2 lastPosition(vec2 uv) {\n"
			+ "  vec2 ndc = uv * 2.0 - 1.0;\n"
			+ "  vec3 p = ToLast * vec3(ndc.x * Tan.x, ndc.y * Tan.y, -1.0);\n"
			+ "  return vec2(p.x / -p.z / Tan.x, p.y / -p.z / Tan.y) * 0.5 + 0.5;\n"
			+ "}\n"
			+ "void main() {\n"
			+ "  vec2 move = (lastPosition(uv) - uv) * Scale + (uv - vec2(0.5)) * Radial;\n"
			+ "  float moved = length(move);\n"
			+ "  if (moved > LONGEST) { move *= LONGEST / moved; }\n"
			+ "  float noise = fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715))));\n"
			+ "  vec3 sum = vec3(0.0);\n"
			+ "  for (int i = 0; i < SAMPLES; i++) {\n"
			+ "    float t = (float(i) + noise) / float(SAMPLES) - 0.5;\n"
			+ "    sum += SAMPLE(Scene, clamp(uv + move * t, 0.0, 1.0)).rgb;\n"
			+ "  }\n"
			+ "  FRAG_OUT(vec4(sum / float(SAMPLES), 1.0))\n"
			+ "}\n";

	private static int makeProgram() {
		//#if MC >= 1.17
		int vs = shader(GL20.GL_VERTEX_SHADER, "#version 150\n"
				+ "out vec2 uv;\n"
				+ "void main() {\n"
				+ "  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));\n"
				+ "  uv = p;\n"
				+ "  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);\n"
				+ "}\n");
		int fs = shader(GL20.GL_FRAGMENT_SHADER, "#version 150\n"
				+ "in vec2 uv;\n"
				+ "out vec4 color;\n"
				+ "#define SAMPLE(s, c) texture(s, c)\n"
				+ "#define FRAG_OUT(v) color = v;\n"
				+ SMEAR);
		//#else
		int vs = shader(GL20.GL_VERTEX_SHADER, "#version 120\n"
				+ "varying vec2 uv;\n"
				+ "void main() {\n"
				+ "  uv = gl_MultiTexCoord0.xy;\n"
				+ "  gl_Position = gl_Vertex;\n"
				+ "}\n");
		int fs = shader(GL20.GL_FRAGMENT_SHADER, "#version 120\n"
				+ "varying vec2 uv;\n"
				+ "#define SAMPLE(s, c) texture2D(s, c)\n"
				+ "#define FRAG_OUT(v) gl_FragColor = v;\n"
				+ SMEAR);
		//#endif
		int p = GL20.glCreateProgram();
		GL20.glAttachShader(p, vs);
		GL20.glAttachShader(p, fs);
		GL20.glLinkProgram(p);
		GL20.glDeleteShader(vs);
		GL20.glDeleteShader(fs);
		if (GL20.glGetProgrami(p, GL20.GL_LINK_STATUS) == 0) {
			throw new IllegalStateException("blur shader: " + GL20.glGetProgramInfoLog(p));
		}
		return p;
	}

	private static int shader(int type, String source) {
		int s = GL20.glCreateShader(type);
		GL20.glShaderSource(s, source);
		GL20.glCompileShader(s);
		if (GL20.glGetShaderi(s, GL20.GL_COMPILE_STATUS) == 0) {
			throw new IllegalStateException("blur shader: " + GL20.glGetShaderInfoLog(s));
		}
		return s;
	}

	private static void setEnabled(int cap, boolean on) {
		if (on) {
			GL11.glEnable(cap);
		} else {
			GL11.glDisable(cap);
		}
	}

	/** The GL state this touches, read before and put back after. */
	private static final class Saved {
		int drawFramebuffer;
		int readFramebuffer;
		final int[] viewport = new int[4];
		int program;
		int vertexArray;
		int activeTexture;
		int texture;
		int sampler;
		boolean blend;
		int srcRgb;
		int dstRgb;
		int srcAlpha;
		int dstAlpha;
		final float[] blendColor = new float[4];
		boolean depthTest;
		boolean cull;
		boolean scissor;
		boolean depthMask;

		static Saved read() {
			Saved s = new Saved();
			s.drawFramebuffer = GL11.glGetInteger(GL30.GL_DRAW_FRAMEBUFFER_BINDING);
			s.readFramebuffer = GL11.glGetInteger(GL30.GL_READ_FRAMEBUFFER_BINDING);
			INTS.clear();
			GL11.glGetIntegerv(GL11.GL_VIEWPORT, INTS);
			for (int i = 0; i < 4; i++) {
				s.viewport[i] = INTS.get(i);
			}
			s.program = GL11.glGetInteger(GL20.GL_CURRENT_PROGRAM);
			s.vertexArray = GL11.glGetInteger(GL30.GL_VERTEX_ARRAY_BINDING);
			s.activeTexture = GL11.glGetInteger(GL13.GL_ACTIVE_TEXTURE);
			GL13.glActiveTexture(GL13.GL_TEXTURE0);
			s.texture = GL11.glGetInteger(GL11.GL_TEXTURE_BINDING_2D);
			s.sampler = Samplers.SUPPORTED ? GL11.glGetInteger(GL33.GL_SAMPLER_BINDING) : 0;
			s.blend = GL11.glIsEnabled(GL11.GL_BLEND);
			s.srcRgb = GL11.glGetInteger(GL14.GL_BLEND_SRC_RGB);
			s.dstRgb = GL11.glGetInteger(GL14.GL_BLEND_DST_RGB);
			s.srcAlpha = GL11.glGetInteger(GL14.GL_BLEND_SRC_ALPHA);
			s.dstAlpha = GL11.glGetInteger(GL14.GL_BLEND_DST_ALPHA);
			FLOATS.clear();
			GL11.glGetFloatv(GL_BLEND_COLOR, FLOATS);
			for (int i = 0; i < 4; i++) {
				s.blendColor[i] = FLOATS.get(i);
			}
			s.depthTest = GL11.glIsEnabled(GL11.GL_DEPTH_TEST);
			s.cull = GL11.glIsEnabled(GL11.GL_CULL_FACE);
			s.scissor = GL11.glIsEnabled(GL11.GL_SCISSOR_TEST);
			s.depthMask = GL11.glGetInteger(GL11.GL_DEPTH_WRITEMASK) != 0;
			return s;
		}

		void restore() {
			GL30.glBindFramebuffer(GL30.GL_DRAW_FRAMEBUFFER, drawFramebuffer);
			GL30.glBindFramebuffer(GL30.GL_READ_FRAMEBUFFER, readFramebuffer);
			GL11.glViewport(viewport[0], viewport[1], viewport[2], viewport[3]);
			GL20.glUseProgram(program);
			GL30.glBindVertexArray(vertexArray);
			GL13.glActiveTexture(GL13.GL_TEXTURE0);
			GL11.glBindTexture(GL11.GL_TEXTURE_2D, texture);
			if (Samplers.SUPPORTED) {
				GL33.glBindSampler(0, sampler);
			}
			GL13.glActiveTexture(activeTexture);
			setEnabled(GL11.GL_BLEND, blend);
			GL14.glBlendFuncSeparate(srcRgb, dstRgb, srcAlpha, dstAlpha);
			GL14.glBlendColor(blendColor[0], blendColor[1], blendColor[2], blendColor[3]);
			setEnabled(GL11.GL_DEPTH_TEST, depthTest);
			setEnabled(GL11.GL_CULL_FACE, cull);
			setEnabled(GL11.GL_SCISSOR_TEST, scissor);
			GL11.glDepthMask(depthMask);
		}
	}
}
//#endif
