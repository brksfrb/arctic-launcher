package com.arcticlauncher.client.looks;

import com.google.gson.JsonParser;
import java.util.EnumMap;
import java.util.Locale;
import java.util.Map;

/**
 * The player's arms and legs cut in two (at the elbow or knee), textured from their skin, for
 * expressive emotes that bend them: while one plays, the game's own limb is hidden and this one is
 * drawn in its place on the same part. The lower half turns by the emote's {@code rightForearm},
 * {@code leftForearm}, {@code rightShin} or {@code leftShin} bone, at the joint.
 *
 * Built as an ordinary cosmetic model (two bones, per-face UVs on the 64×64 skin layout), so every
 * version draws it the way it draws cosmetics.
 */
public final class SkinLimbs {
	public enum Limb {
		RIGHT_ARM(Attach.RIGHT_ARM, "rightForearm", 40, 16, 40, 32, true),
		LEFT_ARM(Attach.LEFT_ARM, "leftForearm", 32, 48, 48, 48, true),
		RIGHT_LEG(Attach.RIGHT_LEG, "rightShin", 0, 16, 0, 32, false),
		LEFT_LEG(Attach.LEFT_LEG, "leftShin", 16, 48, 0, 48, false);

		public final Attach attach;
		/** The emote bone that bends the lower half. */
		public final String joint;
		final int u;
		final int v;
		final int outerU;
		final int outerV;
		final boolean arm;

		Limb(Attach attach, String joint, int u, int v, int outerU, int outerV, boolean arm) {
			this.attach = attach;
			this.joint = joint;
			this.u = u;
			this.v = v;
			this.outerU = outerU;
			this.outerV = outerV;
			this.arm = arm;
		}
	}

	/** The outer skin layer (sleeves, trousers) sits this far out, like the game's. */
	private static final float OUTER = 0.25f;
	private static final int DEPTH = 4;
	private static final int LENGTH = 12;
	private static final int HALF = LENGTH / 2;

	/** Built once each: [slim][outer]. */
	private static final Map<Limb, CuboidModel.Piece[][]> CACHE = new EnumMap<Limb, CuboidModel.Piece[][]>(Limb.class);

	private SkinLimbs() {}

	/** The bent limb's model: {@code slim} arms are 3 wide; {@code outer} adds the skin's outer layer. */
	public static synchronized CuboidModel.Piece piece(Limb limb, boolean slim, boolean outer) {
		CuboidModel.Piece[][] built = CACHE.get(limb);
		if (built == null) {
			built = new CuboidModel.Piece[2][2];
			CACHE.put(limb, built);
		}
		int s = slim && limb.arm ? 1 : 0;
		int o = outer ? 1 : 0;
		if (built[s][o] == null) {
			Geometry geometry = Geometry.parse(new JsonParser().parse(json(limb, s == 1, outer)));
			built[s][o] = CuboidModel.bake(geometry, null).get(0);
		}
		return built[s][o];
	}

	/** Whether {@code animation} bends any limb. */
	public static boolean bends(Animation animation) {
		for (Limb limb : Limb.values()) {
			if (animation.hasBone(limb.joint)) {
				return true;
			}
		}
		return false;
	}

	/** The geometry, in Blockbench's space (y up from the feet), as a cosmetic file would have it. */
	static String json(Limb limb, boolean slim, boolean outer) {
		int width = slim ? 3 : 4;
		// The part's pivot and the limb's box, from the game's player model.
		float pivotX = limb.attach.x;
		float pivotY = 24 - limb.attach.y;
		float x0;
		float top;
		if (limb.arm) {
			x0 = limb == Limb.RIGHT_ARM ? -4 - width : 4;
			top = 24;
		} else {
			x0 = pivotX - 2;
			top = 12;
		}
		float jointX = x0 + width / 2f;
		float jointY = top - HALF;
		StringBuilder upper = new StringBuilder();
		StringBuilder lower = new StringBuilder();
		cube(upper, x0, jointY, width, limb.u, limb.v, 0, 0f);
		cube(lower, x0, top - LENGTH, width, limb.u, limb.v, HALF, 0f);
		if (outer) {
			upper.append(',');
			lower.append(',');
			cube(upper, x0, jointY, width, limb.outerU, limb.outerV, 0, OUTER);
			cube(lower, x0, top - LENGTH, width, limb.outerU, limb.outerV, HALF, OUTER);
		}
		String root = limb.attach.name().toLowerCase(Locale.ROOT).replace("_a", "A").replace("_l", "L");
		return "{\"minecraft:geometry\":[{\"description\":{\"texture_width\":64,\"texture_height\":64},\"bones\":["
				+ "{\"name\":\"" + root + "\",\"pivot\":[" + pivotX + "," + pivotY + ",0],\"cubes\":[" + upper + "]},"
				+ "{\"name\":\"" + limb.joint + "\",\"parent\":\"" + root + "\",\"pivot\":[" + jointX + "," + jointY + ",0],\"cubes\":[" + lower + "]}"
				+ "]}]}";
	}

	/**
	 * One half (6 tall) of a limb box whose full-length box UV starts at (u, v): the sides take the
	 * upper or lower rows ({@code row} 0 or 6); both halves are capped at the joint with the box's
	 * top and bottom.
	 */
	private static void cube(StringBuilder out, float x0, float y0, int w, int u, int v, int row, float inflate) {
		int d = DEPTH;
		int sides = v + d + row;
		out.append("{\"origin\":[").append(x0).append(',').append(y0).append(",-2],\"size\":[").append(w).append(',').append(HALF)
				.append(",4],\"inflate\":").append(inflate).append(",\"uv\":{")
				.append(face("north", u + d, sides, w, HALF)).append(',')
				.append(face("south", u + 2 * d + w, sides, w, HALF)).append(',')
				.append(face("east", u, sides, d, HALF)).append(',')
				.append(face("west", u + d + w, sides, d, HALF)).append(',')
				.append(face("up", u + d, v, w, d)).append(',')
				.append(face("down", u + d + w, v, w, d)).append("}}");
	}

	private static String face(String name, int u, int v, int w, int h) {
		return "\"" + name + "\":{\"uv\":[" + u + "," + v + "],\"uv_size\":[" + w + "," + h + "]}";
	}
}
