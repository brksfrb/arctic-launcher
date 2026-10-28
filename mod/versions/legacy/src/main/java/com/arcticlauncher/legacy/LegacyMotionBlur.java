package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.MotionBlurView;
import com.arcticlauncher.client.config.ClientConfig;
import com.mojang.blaze3d.platform.GLX;
import com.mojang.blaze3d.platform.GlStateManager;
import java.nio.FloatBuffer;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gl.Framebuffer;
import org.lwjgl.BufferUtils;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL20;

/**
 * Motion blur for 1.8.9 to 1.12.2: after the world is drawn (and before the HUD, so the HUD stays sharp) a
 * copy of the frame is smeared along the way the picture moved since the last frame and drawn back (see
 * {@link MotionBlurView}). Mixing in the last frame, as this used to, showed as several copies of the
 * picture when the view turned fast.
 */
public final class LegacyMotionBlur {
	/** The ortho depth range Minecraft's own framebuffer blit uses. */
	private static final double NEAR = 1000.0;
	private static final double FAR = 3000.0;
	private static final float DEPTH = -2000.0f;

	private static final String VERTEX = "#version 120\n"
			+ "varying vec2 uv;\n"
			+ "void main() {\n"
			+ "  uv = gl_MultiTexCoord0.xy;\n"
			+ "  gl_Position = ftransform();\n"
			+ "}\n";

	/**
	 * The smear: each pixel is the average of sixteen samples along the line it moved on since the last frame,
	 * found by turning the pixel's view direction back by the camera's turn. A little noise per pixel moves the
	 * sixteen samples so they don't show as steps.
	 */
	private static final String FRAGMENT = "#version 120\n"
			+ "varying vec2 uv;\n"
			+ "uniform sampler2D Scene;\n"
			+ "uniform mat3 ToLast;\n"
			+ "uniform vec2 Tan;\n"
			+ "uniform float Scale;\n"
			+ "uniform float Radial;\n"
			+ "const int SAMPLES = 16;\n"
			+ "const float LONGEST = 0.25;\n"
			+ "vec2 lastPosition(vec2 at) {\n"
			+ "  vec2 ndc = at * 2.0 - 1.0;\n"
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
			+ "    sum += texture2D(Scene, clamp(uv + move * t, 0.0, 1.0)).rgb;\n"
			+ "  }\n"
			+ "  gl_FragColor = vec4(sum / float(SAMPLES), 1.0);\n"
			+ "}\n";

	private static final MotionBlurView VIEW = new MotionBlurView();
	private static final FloatBuffer MATRIX = BufferUtils.createFloatBuffer(9);

	/** A copy of the frame, read from while the smeared one is drawn over it. */
	private static Framebuffer copy;
	private static int program;
	private static int sceneUniform;
	private static int toLastUniform;
	private static int tanUniform;
	private static int scaleUniform;
	private static int radialUniform;
	private static boolean broken;

	private LegacyMotionBlur() {}

	/** Right after the world is drawn into the main framebuffer. */
	public static void apply() {
		ClientConfig config = ArcticClient.config();
		if (config == null || !config.motionBlur || broken || !GLX.supportsFbo()) {
			release();
			return;
		}
		double[] camera = LegacyHooks.camera();
		MinecraftClient client = MinecraftClient.getInstance();
		Framebuffer main = client.getFramebuffer();
		if (camera == null || main == null) {
			VIEW.reset();
			return;
		}
		int width = main.textureWidth;
		int height = main.textureHeight;
		float fov = camera[5] > 0 ? (float) camera[5] : client.options.fov;
		if (!VIEW.update((float) camera[3], (float) camera[4], camera[0], camera[1], camera[2], fov,
				(float) width / Math.max(1, height), config.motionBlurAmount, System.nanoTime(),
				config.motionBlurMode != 2, config.motionBlurMode != 1)) {
			return;
		}
		try {
			if (copy == null || copy.textureWidth != width || copy.textureHeight != height) {
				if (copy != null) {
					copy.delete();
				}
				copy = new Framebuffer(width, height, false);
			}
			if (program == 0) {
				program = makeProgram();
			}
			draw(main, copy, width, height, false);
			draw(copy, main, width, height, true);
		} catch (RuntimeException e) {
			broken = true;
			ArcticLegacy.LOG.warn("motion blur: {}", e.toString());
		} finally {
			main.bind(true);
		}
	}

	/** Motion blur off (or the window is gone): nothing is kept. */
	private static void release() {
		VIEW.reset();
		if (copy != null) {
			copy.delete();
			copy = null;
		}
	}

	private static int makeProgram() {
		int vs = shader(GL20.GL_VERTEX_SHADER, VERTEX);
		int fs = shader(GL20.GL_FRAGMENT_SHADER, FRAGMENT);
		int p = GL20.glCreateProgram();
		GL20.glAttachShader(p, vs);
		GL20.glAttachShader(p, fs);
		GL20.glLinkProgram(p);
		GL20.glDeleteShader(vs);
		GL20.glDeleteShader(fs);
		if (GL20.glGetProgrami(p, GL20.GL_LINK_STATUS) == 0) {
			throw new IllegalStateException("blur shader: " + GL20.glGetProgramInfoLog(p, 4096));
		}
		sceneUniform = GL20.glGetUniformLocation(p, "Scene");
		toLastUniform = GL20.glGetUniformLocation(p, "ToLast");
		tanUniform = GL20.glGetUniformLocation(p, "Tan");
		scaleUniform = GL20.glGetUniformLocation(p, "Scale");
		radialUniform = GL20.glGetUniformLocation(p, "Radial");
		return p;
	}

	private static int shader(int type, String source) {
		int s = GL20.glCreateShader(type);
		GL20.glShaderSource(s, source);
		GL20.glCompileShader(s);
		if (GL20.glGetShaderi(s, GL20.GL_COMPILE_STATUS) == 0) {
			throw new IllegalStateException("blur shader: " + GL20.glGetShaderInfoLog(s, 4096));
		}
		return s;
	}

	/** Draw {@code source} into {@code target}: as it is, or smeared. */
	private static void draw(Framebuffer source, Framebuffer target, int width, int height, boolean smeared) {
		target.bind(true);
		GlStateManager.matrixMode(GL11.GL_PROJECTION);
		GlStateManager.pushMatrix();
		GlStateManager.loadIdentity();
		GlStateManager.ortho(0.0, width, height, 0.0, NEAR, FAR);
		GlStateManager.matrixMode(GL11.GL_MODELVIEW);
		GlStateManager.pushMatrix();
		GlStateManager.loadIdentity();
		GlStateManager.translate(0.0f, 0.0f, DEPTH);
		GlStateManager.viewport(0, 0, width, height);
		GlStateManager.colorMask(true, true, true, false);
		GlStateManager.disableDepthTest();
		GlStateManager.depthMask(false);
		GlStateManager.disableLighting();
		GlStateManager.disableAlphaTest();
		GlStateManager.enableTexture();
		GlStateManager.disableBlend();
		GlStateManager.color(1f, 1f, 1f, 1f);
		GlStateManager.bindTexture(source.colorAttachment);
		if (smeared) {
			GL20.glUseProgram(program);
			GL20.glUniform1i(sceneUniform, 0);
			MATRIX.clear();
			MATRIX.put(VIEW.toLast).flip();
			GL20.glUniformMatrix3(toLastUniform, false, MATRIX);
			GL20.glUniform2f(tanUniform, VIEW.tanX, VIEW.tanY);
			GL20.glUniform1f(scaleUniform, VIEW.scale);
			GL20.glUniform1f(radialUniform, VIEW.radial);
		}
		GL11.glBegin(GL11.GL_QUADS);
		GL11.glTexCoord2f(0f, 0f);
		GL11.glVertex3f(0f, height, 0f);
		GL11.glTexCoord2f(1f, 0f);
		GL11.glVertex3f(width, height, 0f);
		GL11.glTexCoord2f(1f, 1f);
		GL11.glVertex3f(width, 0f, 0f);
		GL11.glTexCoord2f(0f, 1f);
		GL11.glVertex3f(0f, 0f, 0f);
		GL11.glEnd();
		if (smeared) {
			GL20.glUseProgram(0);
		}
		GlStateManager.bindTexture(0);
		GlStateManager.enableAlphaTest();
		GlStateManager.depthMask(true);
		GlStateManager.enableDepthTest();
		GlStateManager.colorMask(true, true, true, true);
		GlStateManager.matrixMode(GL11.GL_PROJECTION);
		GlStateManager.popMatrix();
		GlStateManager.matrixMode(GL11.GL_MODELVIEW);
		GlStateManager.popMatrix();
	}
}
