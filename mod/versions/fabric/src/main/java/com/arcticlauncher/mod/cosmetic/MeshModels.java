package com.arcticlauncher.mod.cosmetic;

import com.arcticlauncher.client.looks.Attach;
import com.arcticlauncher.client.looks.MeshDraw;
import com.arcticlauncher.client.looks.MeshModel;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.resources.Identifier;

/** Baked sculpted cosmetics ({@link MeshDraw} draws them) with their textures in this game. */
public final class MeshModels {
	public static final class Baked {
		public final MeshModel mesh;
		/** One texture per embedded image. */
		public final Identifier[] textures;
		/** A white texel, for parts painted with vertex colors alone. */
		public final Identifier white;
		public final MeshDraw.Group[] groups;

		Baked(MeshModel mesh, Identifier[] textures, Identifier white, MeshDraw.Group[] groups) {
			this.mesh = mesh;
			this.textures = textures;
			this.white = white;
			this.groups = groups;
		}

		/** The image drawn for a primitive, or the white texel. */
		public Identifier textureOf(int primitive) {
			MeshModel.Primitive p = mesh.primitives.get(primitive);
			int image = p.material < 0 ? -1 : mesh.materials.get(p.material).texture;
			return image < 0 ? white : textures[image];
		}

		/** The image of a primitive's emissive texture, or null. */
		public Identifier emissiveOf(int primitive) {
			MeshModel.Primitive p = mesh.primitives.get(primitive);
			int image = p.material < 0 ? -1 : mesh.materials.get(p.material).emissiveTexture;
			return image < 0 ? null : textures[image];
		}

		public int[] primitives(MeshDraw.Group g) {
			return g.primitives;
		}

		public void emit(int primitive, Attach attach, boolean animate, com.arcticlauncher.client.looks.Xform xf, int light, boolean glow) {
			MeshDraw.emit(mesh, primitive, attach, animate, xf, light, glow);
		}
	}

	private static final Map<String, Baked> BAKED = new ConcurrentHashMap<>();

	private MeshModels() {}

	public static Baked get(String id) {
		return BAKED.get(id);
	}

	public static void bake(String id, MeshModel mesh, Identifier[] textures, Identifier white) {
		BAKED.put(id, new Baked(mesh, textures, white, MeshDraw.groups(mesh)));
	}
}
