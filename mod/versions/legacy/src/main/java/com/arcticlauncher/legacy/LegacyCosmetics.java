package com.arcticlauncher.legacy;

import java.awt.image.BufferedImage;
import java.nio.FloatBuffer;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Animation;
import com.arcticlauncher.client.looks.CuboidModel;
import com.arcticlauncher.client.looks.Geometry;
import com.arcticlauncher.client.looks.MeshDraw;
import com.arcticlauncher.client.looks.MeshModel;
import com.arcticlauncher.client.looks.Xform;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.Identifier;
import org.lwjgl.BufferUtils;
import org.lwjgl.opengl.GL11;

/**
 * 3D cosmetics (cuboid and sculpted) on 1.8.9 to 1.12.2: baked once the
 * textures are uploaded, drawn in immediate mode by {@link LegacyCosmeticLayer}
 * through the same version-neutral drawers as every other version.
 */
final class LegacyCosmetics {
	/** A baked cuboid cosmetic. */
	static final class Cuboid {
		final List<CuboidModel.Piece> pieces;
		final Identifier texture;
		/** Drawn fullbright on top, or null. */
		final Identifier glow;

		Cuboid(List<CuboidModel.Piece> pieces, Identifier texture, Identifier glow) {
			this.pieces = pieces;
			this.texture = texture;
			this.glow = glow;
		}
	}

	/** A baked sculpted cosmetic. */
	static final class Mesh {
		final MeshModel mesh;
		final Identifier[] textures;
		final Identifier white;
		final MeshDraw.Group[] groups;

		Mesh(MeshModel mesh, Identifier[] textures, Identifier white) {
			this.mesh = mesh;
			this.textures = textures;
			this.white = white;
			this.groups = MeshDraw.groups(mesh);
		}

		Identifier textureOf(int primitive) {
			MeshModel.Primitive p = mesh.primitives.get(primitive);
			int image = p.material < 0 ? -1 : mesh.materials.get(p.material).texture;
			return image < 0 ? white : textures[image];
		}

		Identifier emissiveOf(int primitive) {
			MeshModel.Primitive p = mesh.primitives.get(primitive);
			int image = p.material < 0 ? -1 : mesh.materials.get(p.material).emissiveTexture;
			return image < 0 ? null : textures[image];
		}
	}

	private static final Map<String, Cuboid> CUBOIDS = new ConcurrentHashMap<String, Cuboid>();
	private static final Map<String, Mesh> MESHES = new ConcurrentHashMap<String, Mesh>();
	private static boolean whiteRegistered;

	private LegacyCosmetics() {}

	static Cuboid cuboid(String id) {
		return CUBOIDS.get(id);
	}

	static Mesh mesh(String id) {
		return MESHES.get(id);
	}

	/** Upload a cuboid cosmetic's textures and bake its quads (the core calls from a worker thread). */
	static void register(final String id, final Geometry geometry, final Animation idle, byte[] png, byte[] glowPng) {
		final BufferedImage image = LegacyTextures.decode(png);
		final BufferedImage glowImage = glowPng == null ? null : LegacyTextures.decode(glowPng);
		if (image == null) {
			return;
		}
		MinecraftClient.getInstance().submit(new Runnable() {
			@Override
			public void run() {
				try {
					Identifier texture = LegacyTextures.look("cosmetic/" + id);
					if (!LegacyTextures.register(texture, image)) {
						return;
					}
					Identifier glow = null;
					if (glowImage != null) {
						glow = LegacyTextures.look("cosmetic/" + id + "_glow");
						if (!LegacyTextures.register(glow, glowImage)) {
							glow = null;
						}
					}
					CUBOIDS.put(id, new Cuboid(CuboidModel.bake(geometry, idle), texture, glow));
					ArcticClient.looks().cosmetics().ready(id);
				} catch (Exception e) {
					ArcticLegacy.LOG.warn("cosmetic {}: {}", id, e.toString());
				}
			}
		});
	}

	/** Upload a sculpted cosmetic's images and group its primitives. */
	static void register(final String id, final MeshModel mesh) {
		final BufferedImage[] images = new BufferedImage[mesh.images.size()];
		for (int i = 0; i < images.length; i++) {
			images[i] = LegacyTextures.decode(mesh.images.get(i));
			if (images[i] == null) {
				ArcticLegacy.LOG.warn("cosmetic {} (mesh): image {} can't be read", id, i);
				return;
			}
		}
		MinecraftClient.getInstance().submit(new Runnable() {
			@Override
			public void run() {
				try {
					Identifier[] textures = new Identifier[images.length];
					for (int i = 0; i < textures.length; i++) {
						textures[i] = LegacyTextures.look("mesh/" + id + "_" + i);
						if (!LegacyTextures.register(textures[i], images[i])) {
							return;
						}
					}
					Identifier white = LegacyTextures.look("mesh/white");
					if (!whiteRegistered) {
						BufferedImage pixel = new BufferedImage(2, 2, BufferedImage.TYPE_INT_ARGB);
						for (int y = 0; y < 2; y++) {
							for (int x = 0; x < 2; x++) {
								pixel.setRGB(x, y, -1);
							}
						}
						whiteRegistered = LegacyTextures.register(white, pixel);
					}
					MESHES.put(id, new Mesh(mesh, textures, white));
					ArcticClient.looks().cosmetics().ready(id);
				} catch (Exception e) {
					ArcticLegacy.LOG.warn("cosmetic {} (mesh): {}", id, e.toString());
				}
			}
		});
	}

	/**
	 * The pose of the part being drawn on, from OpenGL's current modelview
	 * matrix; the vertices are written in view space and drawn with an identity
	 * modelview (what the drawers' two-sided lighting works in).
	 */
	static final class GlXform extends Xform {
		private static final FloatBuffer BUFFER = BufferUtils.createFloatBuffer(16);
		private final float[] m = new float[16];

		GlXform() {
			BUFFER.clear();
			GL11.glGetFloat(GL11.GL_MODELVIEW_MATRIX, BUFFER);
			BUFFER.get(m);
		}

		@Override
		public void position(float px, float py, float pz) {
			x = m[0] * px + m[4] * py + m[8] * pz + m[12];
			y = m[1] * px + m[5] * py + m[9] * pz + m[13];
			z = m[2] * px + m[6] * py + m[10] * pz + m[14];
		}

		@Override
		public void normal(float nx, float ny, float nz) {
			x = m[0] * nx + m[4] * ny + m[8] * nz;
			y = m[1] * nx + m[5] * ny + m[9] * nz;
			z = m[2] * nx + m[6] * ny + m[10] * nz;
		}

		@Override
		public void vertex(float vx, float vy, float vz, int argb, float u, float v, int light, float nx, float ny, float nz) {
			GL11.glColor4f(((argb >> 16) & 255) / 255f, ((argb >> 8) & 255) / 255f, (argb & 255) / 255f, (argb >>> 24) / 255f);
			GL11.glTexCoord2f(u, v);
			GL11.glNormal3f(nx, ny, nz);
			GL11.glVertex3f(vx, vy, vz);
		}
	}
}
