package com.arcticlauncher.client.looks;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * A cosmetic's shape, from a Blockbench "Bedrock geometry" file: bones
 * (with pivots, rotations and parents) holding box-UV cubes. Everything is
 * checked while parsing; a file outside the limits is refused whole.
 */
public final class Geometry {
	public static final int MAX_BONES = 64;
	public static final int MAX_CUBES = 256;
	/** Coordinates and sizes stay within this many pixels of the origin. */
	private static final float MAX_COORD = 256f;
	private static final float MAX_ROTATION = 360f;
	private static final int MAX_TEXTURE = 512;
	/** Largest texture side for a cosmetic. */
	public static final int MAX_TEXTURE_SIDE = MAX_TEXTURE;
	private static final int MAX_NAME = 64;

	public static final class Cube {
		public final float[] origin;
		public final float[] size;
		public final int u;
		public final int v;
		public final float inflate;
		public final boolean mirror;

		Cube(float[] origin, float[] size, int u, int v, float inflate, boolean mirror) {
			this.origin = origin;
			this.size = size;
			this.u = u;
			this.v = v;
			this.inflate = inflate;
			this.mirror = mirror;
		}
	}

	public static final class Bone {
		public final String name;
		/** Parent bone name, or null for a root bone. */
		public final String parent;
		public final float[] pivot;
		/** Degrees, as Blockbench shows them. */
		public final float[] rotation;
		public final List<Cube> cubes;

		Bone(String name, String parent, float[] pivot, float[] rotation, List<Cube> cubes) {
			this.name = name;
			this.parent = parent;
			this.pivot = pivot;
			this.rotation = rotation;
			this.cubes = cubes;
		}
	}

	public final int textureWidth;
	public final int textureHeight;
	public final List<Bone> bones;

	private Geometry(int textureWidth, int textureHeight, List<Bone> bones) {
		this.textureWidth = textureWidth;
		this.textureHeight = textureHeight;
		this.bones = bones;
	}

	/** Parse a geometry file; throws with a reason when it isn't one we accept. */
	public static Geometry parse(JsonElement json) {
		JsonObject geometry = first(object(json).get("minecraft:geometry"));
		JsonObject description = object(geometry.get("description"));
		int w = texture(description.get("texture_width"));
		int h = texture(description.get("texture_height"));
		JsonArray bonesJson = array(geometry.get("bones"));
		if (bonesJson.size() == 0 || bonesJson.size() > MAX_BONES) {
			throw new IllegalArgumentException("needs 1 to " + MAX_BONES + " bones");
		}
		List<Bone> bones = new ArrayList<Bone>();
		List<String> names = new ArrayList<String>();
		int cubes = 0;
		for (JsonElement e : bonesJson) {
			JsonObject b = object(e);
			String name = name(b.get("name"));
			if (names.contains(name)) {
				throw new IllegalArgumentException("two bones called " + name);
			}
			names.add(name);
			String parent = b.has("parent") ? name(b.get("parent")) : null;
			List<Cube> list = new ArrayList<Cube>();
			if (b.has("cubes")) {
				for (JsonElement c : array(b.get("cubes"))) {
					if (++cubes > MAX_CUBES) {
						throw new IllegalArgumentException("more than " + MAX_CUBES + " cubes");
					}
					list.add(cube(object(c)));
				}
			}
			bones.add(new Bone(name, parent, vec(b.get("pivot"), MAX_COORD), optionalVec(b.get("rotation"), MAX_ROTATION),
					Collections.unmodifiableList(list)));
		}
		for (Bone b : bones) {
			if (b.parent != null && !names.contains(b.parent)) {
				throw new IllegalArgumentException("bone " + b.name + " has a missing parent " + b.parent);
			}
		}
		checkNoCycles(bones);
		return new Geometry(w, h, Collections.unmodifiableList(bones));
	}

	public Bone bone(String name) {
		for (Bone b : bones) {
			if (b.name.equals(name)) {
				return b;
			}
		}
		return null;
	}

	private static Cube cube(JsonObject c) {
		float[] origin = vec(c.get("origin"), MAX_COORD);
		float[] size = vec(c.get("size"), MAX_COORD);
		for (float s : size) {
			if (s < 0) {
				throw new IllegalArgumentException("negative cube size");
			}
		}
		JsonElement uvJson = c.get("uv");
		if (uvJson == null || !uvJson.isJsonArray()) {
			throw new IllegalArgumentException("cubes need box UV ([u, v]); per-face UV isn't supported");
		}
		JsonArray uv = uvJson.getAsJsonArray();
		if (uv.size() != 2) {
			throw new IllegalArgumentException("uv must be [u, v]");
		}
		int u = (int) number(uv.get(0), MAX_TEXTURE);
		int v = (int) number(uv.get(1), MAX_TEXTURE);
		float inflate = c.has("inflate") ? number(c.get("inflate"), 16) : 0f;
		boolean mirror = c.has("mirror") && c.get("mirror").isJsonPrimitive() && c.get("mirror").getAsBoolean();
		return new Cube(origin, size, u, v, inflate, mirror);
	}

	/** Parents must lead to a root (no loops). */
	private static void checkNoCycles(List<Bone> bones) {
		for (Bone b : bones) {
			Bone at = b;
			for (int steps = 0; at.parent != null; steps++) {
				if (steps > bones.size()) {
					throw new IllegalArgumentException("bone parents loop around " + b.name);
				}
				at = find(bones, at.parent);
			}
		}
	}

	private static Bone find(List<Bone> bones, String name) {
		for (Bone b : bones) {
			if (b.name.equals(name)) {
				return b;
			}
		}
		throw new IllegalArgumentException("missing bone " + name);
	}

	// ---- Checked JSON reading ---------------------------------------------------

	static JsonObject object(JsonElement e) {
		if (e == null || !e.isJsonObject()) {
			throw new IllegalArgumentException("expected an object");
		}
		return e.getAsJsonObject();
	}

	static JsonArray array(JsonElement e) {
		if (e == null || !e.isJsonArray()) {
			throw new IllegalArgumentException("expected a list");
		}
		return e.getAsJsonArray();
	}

	private static JsonObject first(JsonElement e) {
		JsonArray a = array(e);
		if (a.size() == 0) {
			throw new IllegalArgumentException("no geometry");
		}
		return object(a.get(0));
	}

	/** A finite number with |n| <= limit. */
	static float number(JsonElement e, float limit) {
		if (e == null || !e.isJsonPrimitive() || !e.getAsJsonPrimitive().isNumber()) {
			throw new IllegalArgumentException("expected a number");
		}
		double d = e.getAsDouble();
		if (Double.isNaN(d) || Double.isInfinite(d) || Math.abs(d) > limit) {
			throw new IllegalArgumentException("number out of range: " + d);
		}
		return (float) d;
	}

	static float[] vec(JsonElement e, float limit) {
		JsonArray a = array(e);
		if (a.size() != 3) {
			throw new IllegalArgumentException("expected [x, y, z]");
		}
		return new float[] {number(a.get(0), limit), number(a.get(1), limit), number(a.get(2), limit)};
	}

	private static float[] optionalVec(JsonElement e, float limit) {
		return e == null ? new float[3] : vec(e, limit);
	}

	private static int texture(JsonElement e) {
		int side = (int) number(e, MAX_TEXTURE);
		if (side <= 0 || (side & (side - 1)) != 0) {
			throw new IllegalArgumentException("texture sides must be powers of two up to " + MAX_TEXTURE);
		}
		return side;
	}

	private static String name(JsonElement e) {
		if (e == null || !e.isJsonPrimitive() || !e.getAsJsonPrimitive().isString()) {
			throw new IllegalArgumentException("expected a name");
		}
		String s = e.getAsString();
		if (s.isEmpty() || s.length() > MAX_NAME) {
			throw new IllegalArgumentException("bad bone name");
		}
		return s;
	}
}
