package com.arcticlauncher.mod;

//#if MC < 26.1
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.mojang.blaze3d.pipeline.RenderTarget;
import java.nio.IntBuffer;
import java.nio.FloatBuffer;
import net.minecraft.client.Minecraft;
import org.lwjgl.BufferUtils;
import org.lwjgl.opengl.GL;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL13;
import org.lwjgl.opengl.GL14;
import org.lwjgl.opengl.GL20;
import org.lwjgl.opengl.GL30;

/**
 * Motion blur before 26.1 (26.1+ uses a post effect in the client's pack): right after the world is
 * drawn, the previous frame is blended over the new one at the chosen strength, and the result is
 * kept for the next frame. Done with plain OpenGL on textures and framebuffers of its own, so it
 * doesn't depend on Minecraft's renderer (which changes nearly every version); every bit of GL state
 * it touches is put back, so Minecraft's state tracking stays right.
 */
public final class MotionBlurGL {
	/** Strength 1-4 as how much of the previous frame stays (the same as the 26.1+ effect). */
	private static final float[] STRENGTHS = {0.5f, 0.65f, 0.78f, 0.88f};
	/** GL_BLEND_COLOR (not among LWJGL's GL14 constants). */
	private static final int GL_BLEND_COLOR = 0x8005;

	private static int width;
	private static int height;
	/** The previous frame. */
	private static int prevTexture;
	private static int prevFramebuffer;
	/** A framebuffer on the game's own frame texture (made for whichever texture is current). */
	private static int mainFramebuffer;
	private static int mainTexture;
	private static boolean hasPrevious;
	private static int program;
	private static int prevUniform;
	private static int vertexArray;
	private static boolean broken;

	private static final IntBuffer INTS = BufferUtils.createIntBuffer(16);
	private static final FloatBuffer FLOATS = BufferUtils.createFloatBuffer(16);

	private MotionBlurGL() {}

	/** After the world is drawn, before the HUD. */
	public static void apply() {
		ClientConfig c = ArcticClient.config();
		Minecraft mc = Minecraft.getInstance();
		if (c == null || !c.motionBlur || mc.level == null || broken) {
			hasPrevious = false;
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
		Saved saved = Saved.read();
		try {
			prepare(target.width, target.height, texture);
			if (hasPrevious) {
				float strength = STRENGTHS[Math.max(1, Math.min(STRENGTHS.length, c.motionBlurStrength)) - 1];
				blend(strength);
			}
			// Keep this frame for the next one.
			GL30.glBindFramebuffer(GL30.GL_READ_FRAMEBUFFER, mainFramebuffer);
			GL30.glBindFramebuffer(GL30.GL_DRAW_FRAMEBUFFER, prevFramebuffer);
			GL30.glBlitFramebuffer(0, 0, width, height, 0, 0, width, height, GL11.GL_COLOR_BUFFER_BIT, GL11.GL_NEAREST);
			hasPrevious = true;
		} catch (RuntimeException e) {
			broken = true;
			ArcticMod.LOG.warn("motion blur: {}", e.toString());
		} finally {
			saved.restore();
		}
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
			GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_NEAREST);
			GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_NEAREST);
			GL11.glTexImage2D(GL11.GL_TEXTURE_2D, 0, GL11.GL_RGBA8, w, h, 0, GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE, (java.nio.ByteBuffer) null);
			if (prevFramebuffer == 0) {
				prevFramebuffer = GL30.glGenFramebuffers();
			}
			GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, prevFramebuffer);
			GL30.glFramebufferTexture2D(GL30.GL_FRAMEBUFFER, GL30.GL_COLOR_ATTACHMENT0, GL11.GL_TEXTURE_2D, prevTexture, 0);
			width = w;
			height = h;
			hasPrevious = false;
		}
		if (mainFramebuffer == 0) {
			mainFramebuffer = GL30.glGenFramebuffers();
		}
		if (texture != mainTexture) {
			GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, mainFramebuffer);
			GL30.glFramebufferTexture2D(GL30.GL_FRAMEBUFFER, GL30.GL_COLOR_ATTACHMENT0, GL11.GL_TEXTURE_2D, texture, 0);
			mainTexture = texture;
			hasPrevious = false;
		}
	}

	/** The previous frame over the new one: new * (1 - strength) + previous * strength. */
	private static void blend(float strength) {
		GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, mainFramebuffer);
		GL11.glViewport(0, 0, width, height);
		GL11.glDisable(GL11.GL_DEPTH_TEST);
		GL11.glDisable(GL11.GL_CULL_FACE);
		GL11.glDisable(GL11.GL_SCISSOR_TEST);
		GL11.glDepthMask(false);
		GL11.glEnable(GL11.GL_BLEND);
		// A constant blend: the frame textures' alpha isn't dependable. Destination alpha stays.
		GL14.glBlendColor(0f, 0f, 0f, strength);
		GL14.glBlendFuncSeparate(GL14.GL_CONSTANT_ALPHA, GL14.GL_ONE_MINUS_CONSTANT_ALPHA, GL11.GL_ZERO, GL11.GL_ONE);
		GL13.glActiveTexture(GL13.GL_TEXTURE0);
		GL11.glBindTexture(GL11.GL_TEXTURE_2D, prevTexture);
		//#if MC >= 1.17
		// 1.17+ runs a core OpenGL 3.2 context: a fullscreen triangle with a tiny shader.
		if (program == 0) {
			program = makeProgram();
			prevUniform = GL20.glGetUniformLocation(program, "Prev");
			vertexArray = GL30.glGenVertexArrays();
		}
		GL20.glUseProgram(program);
		GL20.glUniform1i(prevUniform, 0);
		GL30.glBindVertexArray(vertexArray);
		GL11.glDrawArrays(GL11.GL_TRIANGLES, 0, 3);
		//#else
		// Before 1.17 the context is a compatibility one: a plain textured quad.
		GL20.glUseProgram(0);
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

	//#if MC >= 1.17
	private static int makeProgram() {
		int vs = shader(GL20.GL_VERTEX_SHADER, "#version 150\n"
				+ "out vec2 uv;\n"
				+ "void main() {\n"
				+ "  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));\n"
				+ "  uv = p;\n"
				+ "  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);\n"
				+ "}\n");
		int fs = shader(GL20.GL_FRAGMENT_SHADER, "#version 150\n"
				+ "uniform sampler2D Prev;\n"
				+ "in vec2 uv;\n"
				+ "out vec4 color;\n"
				+ "void main() {\n"
				+ "  color = vec4(texture(Prev, uv).rgb, 1.0);\n"
				+ "}\n");
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
	//#endif

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
