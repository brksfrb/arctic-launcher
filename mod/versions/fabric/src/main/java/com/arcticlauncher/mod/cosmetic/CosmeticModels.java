package com.arcticlauncher.mod.cosmetic;

import com.arcticlauncher.client.looks.Animation;
import com.arcticlauncher.client.looks.CuboidModel;
import com.arcticlauncher.client.looks.Geometry;
import com.arcticlauncher.client.looks.Xform;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.resources.Identifier;

/** Baked cuboid cosmetics (the geometry is {@link CuboidModel}'s) with their textures in this game. */
public final class CosmeticModels {
	/** A baked cosmetic: its pieces and textures. */
	public static final class Baked {
		public final List<CuboidModel.Piece> pieces;
		public final Identifier texture;
		/** Drawn fullbright on top, or null. */
		public final Identifier glow;

		Baked(List<CuboidModel.Piece> pieces, Identifier texture, Identifier glow) {
			this.pieces = pieces;
			this.texture = texture;
			this.glow = glow;
		}
	}

	private static final Map<String, Baked> BAKED = new ConcurrentHashMap<>();

	private CosmeticModels() {}

	public static Baked get(String id) {
		return BAKED.get(id);
	}

	/** Build the quads (render thread). */
	public static void bake(String id, Geometry geometry, Animation idle, Identifier texture, Identifier glow) {
		BAKED.put(id, new Baked(CuboidModel.bake(geometry, idle), texture, glow));
	}

	/** Packed light for parts that glow. */
	public static int fullBright() {
		return Xform.FULL_BRIGHT;
	}
}
