package com.arcticlauncher.polonium;

import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.font.TextRenderable;
import net.minecraft.client.renderer.rendertype.RenderType;
import org.joml.Matrix4f;
import org.joml.Matrix4fc;

/**
 * A laid-out name tag's quads, recorded the first time it's drawn and
 * replayed after. Drawing text goes glyph by glyph through styles, glyph
 * lookups and a vertex builder per glyph; a tag's quads don't change while
 * its layout doesn't, so they're recorded once in the tag's own space (each
 * vertex's position, color and texture coordinates, exactly as the glyphs
 * write them) and written again each frame with that frame's pose and light.
 * Runs keep the glyphs' order and render types, so the game's buffers get
 * the same vertices in the same order.
 *
 * The recording lives on the laid-out text (see the PreparedText mixin), one
 * per way it's drawn. Render thread only.
 */
public final class GlyphRuns {
	private static final Matrix4fc IDENTITY = new Matrix4f();

	/** Room on a laid-out text for its recorded runs, per display mode (added by a mixin). */
	public interface Holder {
		Run[][] polonium$runs();
	}

	private GlyphRuns() {}

	/** The text's runs for this display mode, recorded now if they weren't yet. */
	public static Run[] runs(Font.PreparedText text, Font.DisplayMode mode) {
		Run[][] kept = ((Holder) text).polonium$runs();
		Run[] runs = kept[mode.ordinal()];
		if (runs == null) {
			runs = record(text, mode);
			kept[mode.ordinal()] = runs;
		}
		return runs;
	}

	private static Run[] record(Font.PreparedText text, Font.DisplayMode mode) {
		List<Run> runs = new ArrayList<>();
		Recorder recorder = new Recorder();
		text.visit(new Font.GlyphVisitor() {
			@Override
			public void acceptRenderable(TextRenderable renderable) {
				RenderType type = renderable.renderType(mode);
				if (recorder.type != type) {
					recorder.finish(runs);
					recorder.type = type;
				}
				renderable.render(IDENTITY, recorder, 0, false);
			}
		});
		recorder.finish(runs);
		return runs.toArray(new Run[0]);
	}

	/** One render type's consecutive quads, in the text's own space. */
	public static final class Run implements TextRenderable {
		private final RenderType type;
		/** Per vertex: x, y, z, u, v. */
		private final float[] vertices;
		private final int[] colors;
		/** The vertices packed for the GPU name tag stream, made on first use. */
		private byte[] packed;

		Run(RenderType type, float[] vertices, int[] colors) {
			this.type = type;
			this.vertices = vertices;
			this.colors = colors;
		}

		@Override
		public void render(Matrix4fc pose, VertexConsumer buffer, int packedLightCoords, boolean flat) {
			for (int i = 0, v = 0; i < colors.length; i++, v += 5) {
				buffer.addVertex(pose, vertices[v], vertices[v + 1], vertices[v + 2])
						.setColor(colors[i])
						.setUv(vertices[v + 3], vertices[v + 4])
						.setLight(packedLightCoords);
			}
		}

		/** The vertices as {@link com.arcticlauncher.polonium.gpu.GpuText} streams them. */
		public byte[] packed() {
			if (packed == null) {
				packed = com.arcticlauncher.polonium.gpu.GpuText.pack(vertices, colors);
			}
			return packed;
		}

		@Override
		public RenderType renderType(Font.DisplayMode displayMode) {
			return type;
		}

		@Override
		public GpuTextureView textureView() {
			return null;
		}

		@Override
		public RenderPipeline guiPipeline() {
			return null;
		}

		@Override
		public float left() {
			return 0;
		}

		@Override
		public float top() {
			return 0;
		}

		@Override
		public float right() {
			return 0;
		}

		@Override
		public float bottom() {
			return 0;
		}
	}

	/** Takes what glyphs write: position, color and texture coordinates per vertex. */
	private static final class Recorder implements VertexConsumer {
		RenderType type;
		private float[] vertices = new float[5 * 64];
		private int[] colors = new int[64];
		private int count;

		void finish(List<Run> runs) {
			if (type != null && count > 0) {
				runs.add(new Run(type, Arrays.copyOf(vertices, count * 5), Arrays.copyOf(colors, count)));
			}
			count = 0;
		}

		@Override
		public VertexConsumer addVertex(float x, float y, float z) {
			if (count == colors.length) {
				colors = Arrays.copyOf(colors, count * 2);
				vertices = Arrays.copyOf(vertices, count * 10);
			}
			int v = count * 5;
			vertices[v] = x;
			vertices[v + 1] = y;
			vertices[v + 2] = z;
			colors[count] = -1;
			count++;
			return this;
		}

		@Override
		public VertexConsumer setColor(int r, int g, int b, int a) {
			colors[count - 1] = a << 24 | r << 16 | g << 8 | b;
			return this;
		}

		@Override
		public VertexConsumer setColor(int color) {
			colors[count - 1] = color;
			return this;
		}

		@Override
		public VertexConsumer setUv(float u, float v) {
			int at = (count - 1) * 5;
			vertices[at + 3] = u;
			vertices[at + 4] = v;
			return this;
		}

		@Override
		public VertexConsumer setUv1(int u, int v) {
			return this;
		}

		@Override
		public VertexConsumer setUv2(int u, int v) {
			return this;
		}

		@Override
		public VertexConsumer setNormal(float x, float y, float z) {
			return this;
		}

		@Override
		public VertexConsumer setLineWidth(float width) {
			return this;
		}
	}
}
