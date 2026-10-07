package com.arcticlauncher.client.looks;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * A sculpted cosmetic: a restricted glTF 2.0 (GLB) file, read with exactly
 * the rules of the launcher's {@code arctic-mesh} crate (the content guide
 * describes the format). Positions are in model pixels, in the space of the
 * cuboid cosmetics (feet at the origin, y up, the player faces -z, +x is the
 * player's left). Everything is checked before anything is allocated for it;
 * a file outside the limits is refused whole.
 */
public final class MeshModel {
	public static final int MAX_BYTES = 1024 * 1024;
	public static final int MAX_NODES = 32;
	public static final int MAX_PRIMITIVES = 16;
	public static final int MAX_TRIANGLES = 24_000;
	public static final int MAX_MATERIALS = 6;
	public static final int MAX_IMAGES = 6;
	public static final int MAX_TEXTURE_SIDE = 1024;
	public static final float MAX_ANIMATION_SECS = 10f;
	private static final int MAX_KEYFRAMES = 256;
	private static final int MAX_CHANNELS = 16;
	private static final float MAX_COORD = 256f;
	private static final int MAX_ACCESSOR = 3 * MAX_TRIANGLES;
	private static final int GLB_MAGIC = 0x46546C67;
	private static final int CHUNK_JSON = 0x4E4F534A;
	private static final int CHUNK_BIN = 0x004E4942;
	private static final Gson GSON = new Gson();

	public static final int ALPHA_OPAQUE = 0;
	public static final int ALPHA_MASK = 1;
	public static final int ALPHA_BLEND = 2;
	public static final int TRANSLATION = 0;
	public static final int ROTATION = 1;
	public static final int SCALE = 2;

	public static final class Node {
		public final String name;
		/** Index of the parent (parents come first), or -1. */
		public final int parent;
		public final float[] translation;
		/** Unit quaternion x, y, z, w. */
		public final float[] rotation;
		public final float[] scale;

		Node(String name, int parent, float[] translation, float[] rotation, float[] scale) {
			this.name = name;
			this.parent = parent;
			this.translation = translation;
			this.rotation = rotation;
			this.scale = scale;
		}
	}

	public static final class Material {
		public float[] baseColor = {1, 1, 1, 1};
		/** Index into {@link MeshModel#images}, or -1. */
		public int texture = -1;
		public int emissiveTexture = -1;
		public float[] emissiveFactor = {0, 0, 0};
		public int alpha = ALPHA_OPAQUE;
		public boolean doubleSided;
	}

	/** Triangles ready to draw: each vertex has a normal, a texture coordinate and a color. */
	public static final class Primitive {
		public final int node;
		/** Index into {@link MeshModel#materials}, or -1. */
		public final int material;
		/** x, y, z per vertex. */
		public final float[] positions;
		public final float[] normals;
		/** u, v per vertex. */
		public final float[] uvs;
		/** r, g, b, a per vertex. */
		public final float[] colors;
		/** Three per triangle. */
		public final int[] indices;

		Primitive(int node, int material, float[] positions, float[] normals, float[] uvs, float[] colors, int[] indices) {
			this.node = node;
			this.material = material;
			this.positions = positions;
			this.normals = normals;
			this.uvs = uvs;
			this.colors = colors;
			this.indices = indices;
		}
	}

	public static final class Channel {
		public final int node;
		public final int path;
		public final boolean step;
		public final float[] times;
		/** Four per key (a vec3 keeps a 0 in the last place). */
		public final float[] values;

		Channel(int node, int path, boolean step, float[] times, float[] values) {
			this.node = node;
			this.path = path;
			this.step = step;
			this.times = times;
			this.values = values;
		}

		/** The value at {@code time}, held before the first and after the last key. */
		void sample(float time, float[] out) {
			int n = times.length;
			if (n == 1 || time <= times[0]) {
				System.arraycopy(values, 0, out, 0, 4);
				return;
			}
			if (time >= times[n - 1]) {
				System.arraycopy(values, (n - 1) * 4, out, 0, 4);
				return;
			}
			int i = 1;
			while (times[i] <= time) {
				i++;
			}
			float t0 = times[i - 1];
			float t1 = times[i];
			if (step || t1 <= t0) {
				System.arraycopy(values, (i - 1) * 4, out, 0, 4);
				return;
			}
			float k = (time - t0) / (t1 - t0);
			int a = (i - 1) * 4;
			int b = i * 4;
			if (path != ROTATION) {
				for (int c = 0; c < 4; c++) {
					out[c] = values[a + c] + (values[b + c] - values[a + c]) * k;
				}
				return;
			}
			float dot = 0;
			for (int c = 0; c < 4; c++) {
				dot += values[a + c] * values[b + c];
			}
			float sign = dot < 0 ? -1f : 1f;
			dot = Math.abs(dot);
			float wa;
			float wb;
			if (dot > 0.9995f) {
				wa = 1 - k;
				wb = k;
			} else {
				float theta = (float) Math.acos(Math.min(1f, dot));
				float s = (float) Math.sin(theta);
				wa = (float) Math.sin((1 - k) * theta) / s;
				wb = (float) Math.sin(k * theta) / s;
			}
			float len = 0;
			for (int c = 0; c < 4; c++) {
				out[c] = values[a + c] * wa + sign * values[b + c] * wb;
				len += out[c] * out[c];
			}
			len = Math.max((float) Math.sqrt(len), 1e-9f);
			for (int c = 0; c < 4; c++) {
				out[c] /= len;
			}
		}
	}

	public final List<Node> nodes;
	public final List<Primitive> primitives;
	public final List<Material> materials;
	/** Embedded PNG files. */
	public final List<byte[]> images;
	/** Idle animation channels (empty: none) and its length in seconds. */
	public final List<Channel> channels;
	public final float animationLength;
	/** The travelling light across the surface, or null. */
	public final Sheen sheen;

	/**
	 * A soft band of light that sweeps along an axis once per period, tinted and
	 * added on top of the model (the scene's {@code extras.arctic.sheen}).
	 */
	public static final class Sheen {
		public final float period;
		public final float[] tint;
		public final float strength;
		public final float width;
		public final float[] axis;
		public final float origin;
		public final float length;
		public final float[] skewAxis;
		public final float skew;
		public final float skewOrigin;
		public final boolean skewAbs;

		Sheen(float period, float[] tint, float strength, float width, float[] axis, float origin, float length, float[] skewAxis, float skew,
				float skewOrigin, boolean skewAbs) {
			this.period = period;
			this.tint = tint;
			this.strength = strength;
			this.width = width;
			this.axis = axis;
			this.origin = origin;
			this.length = length;
			this.skewAxis = skewAxis;
			this.skew = skew;
			this.skewOrigin = skewOrigin;
			this.skewAbs = skewAbs;
		}

		/** Brightness (0 to 1) at model-space point (x, y, z) whose surface faces {@code facingZ}, {@code t} seconds into the loop. */
		public float at(float x, float y, float z, float facingZ, float t) {
			float sweep = (t / period) % 1f;
			if (sweep < 0) {
				sweep += 1f;
			}
			float tilt = skewAxis[0] * x + skewAxis[1] * y + skewAxis[2] * z - skewOrigin;
			if (skewAbs) {
				tilt = Math.abs(tilt);
			}
			float place = (axis[0] * x + axis[1] * y + axis[2] * z - origin) / length + skew * tilt;
			float d = (place - sweep) / width;
			float band = (float) Math.exp(-(d * d));
			float fade = smooth(0f, 0.12f, sweep) * (1f - smooth(0.88f, 1f, sweep));
			float facing = 0.62f + 0.38f * Math.abs(facingZ);
			return Math.max(0f, Math.min(1f, band * fade * facing * strength));
		}

		private static float smooth(float a, float b, float x) {
			float t = Math.max(0f, Math.min(1f, (x - a) / (b - a)));
			return t * t * (3f - 2f * t);
		}
	}

	private MeshModel(List<Node> nodes, List<Primitive> primitives, List<Material> materials, List<byte[]> images, List<Channel> channels,
			float animationLength, Sheen sheen) {
		this.sheen = sheen;
		this.nodes = Collections.unmodifiableList(nodes);
		this.primitives = Collections.unmodifiableList(primitives);
		this.materials = Collections.unmodifiableList(materials);
		this.images = Collections.unmodifiableList(images);
		this.channels = Collections.unmodifiableList(channels);
		this.animationLength = animationLength;
	}

	public int triangles() {
		int n = 0;
		for (Primitive p : primitives) {
			n += p.indices.length / 3;
		}
		return n;
	}

	/**
	 * Each node's matrix from the model's origin (row-major, 16 floats) at
	 * {@code time} seconds into the animation; negative: the authored rest
	 * pose, with the animation off.
	 */
	public float[][] world(float time) {
		float[][] out = new float[nodes.size()][];
		float[] value = new float[4];
		float t = channels.isEmpty() || time < 0 || animationLength <= 0 ? -1f : time % animationLength;
		for (int i = 0; i < out.length; i++) {
			Node node = nodes.get(i);
			float[] tr = node.translation;
			float[] rot = node.rotation;
			float[] sc = node.scale;
			if (t >= 0) {
				for (Channel c : channels) {
					if (c.node != i) {
						continue;
					}
					c.sample(t, value);
					if (c.path == TRANSLATION) {
						tr = new float[] {value[0], value[1], value[2]};
					} else if (c.path == SCALE) {
						sc = new float[] {value[0], value[1], value[2]};
					} else {
						rot = value.clone();
					}
				}
			}
			float[] local = trs(tr, rot, sc);
			out[i] = node.parent >= 0 ? mul(out[node.parent], local) : local;
		}
		return out;
	}

	public static float[] trs(float[] t, float[] q, float[] s) {
		float x = q[0];
		float y = q[1];
		float z = q[2];
		float w = q[3];
		return new float[] {
				(1 - 2 * (y * y + z * z)) * s[0], 2 * (x * y - z * w) * s[1], 2 * (x * z + y * w) * s[2], t[0],
				2 * (x * y + z * w) * s[0], (1 - 2 * (x * x + z * z)) * s[1], 2 * (y * z - x * w) * s[2], t[1],
				2 * (x * z - y * w) * s[0], 2 * (y * z + x * w) * s[1], (1 - 2 * (x * x + y * y)) * s[2], t[2],
				0, 0, 0, 1};
	}

	public static float[] mul(float[] a, float[] b) {
		float[] out = new float[16];
		for (int i = 0; i < 4; i++) {
			for (int j = 0; j < 4; j++) {
				float v = 0;
				for (int k = 0; k < 4; k++) {
					v += a[i * 4 + k] * b[k * 4 + j];
				}
				out[i * 4 + j] = v;
			}
		}
		return out;
	}

	// ---- Reading ----------------------------------------------------------------------------

	/** Parse and check a GLB file; throws with the reason when it isn't one we accept. */
	public static MeshModel parse(byte[] bytes) {
		if (bytes.length > MAX_BYTES) {
			throw new IllegalArgumentException("larger than " + MAX_BYTES + " bytes");
		}
		ByteBuffer b = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
		if (bytes.length < 20 || b.getInt(0) != GLB_MAGIC) {
			throw new IllegalArgumentException("not a .glb file");
		}
		if (b.getInt(4) != 2) {
			throw new IllegalArgumentException("only glTF 2.0 is supported");
		}
		if (b.getInt(8) != bytes.length) {
			throw new IllegalArgumentException("the header's length is wrong");
		}
		byte[] json = null;
		int binAt = 0;
		int binLen = 0;
		int at = 12;
		while (at < bytes.length) {
			if (at + 8 > bytes.length) {
				throw new IllegalArgumentException("file ends early");
			}
			int len = b.getInt(at);
			int kind = b.getInt(at + 4);
			if (len < 0 || at + 8 + (long) len > bytes.length) {
				throw new IllegalArgumentException("a chunk runs past the end");
			}
			if (kind == CHUNK_JSON && json == null) {
				json = Arrays.copyOfRange(bytes, at + 8, at + 8 + len);
			} else if (kind == CHUNK_BIN && at > 12) {
				binAt = at + 8;
				binLen = len;
			} else {
				throw new IllegalArgumentException("unexpected chunk");
			}
			at += 8 + (len + 3) / 4 * 4;
		}
		if (json == null) {
			throw new IllegalArgumentException("no JSON chunk");
		}
		JsonObject doc = GSON.fromJson(new String(json, StandardCharsets.UTF_8), JsonObject.class);
		return new Reader(doc, ByteBuffer.wrap(bytes, binAt, binLen).slice().order(ByteOrder.LITTLE_ENDIAN), bytes, binAt, binLen).read();
	}

	private static final class Reader {
		final JsonObject doc;
		final ByteBuffer bin;
		final byte[] raw;
		final int binAt;
		final int binLen;

		Reader(JsonObject doc, ByteBuffer bin, byte[] raw, int binAt, int binLen) {
			this.doc = doc;
			this.bin = bin;
			this.raw = raw;
			this.binAt = binAt;
			this.binLen = binLen;
		}

		MeshModel read() {
			JsonObject asset = doc.has("asset") ? Geometry.object(doc.get("asset")) : null;
			if (asset == null || !asset.has("version") || !"2.0".equals(asset.get("version").getAsString())) {
				throw new IllegalArgumentException("asset.version must be 2.0");
			}
			if (size("extensionsUsed") > 0 || size("extensionsRequired") > 0) {
				throw new IllegalArgumentException("glTF extensions are not supported");
			}
			for (String key : new String[] {"cameras", "skins"}) {
				if (size(key) > 0) {
					throw new IllegalArgumentException(key + " are not supported");
				}
			}
			if (size("buffers") > 1) {
				throw new IllegalArgumentException("only the file's own binary chunk may hold data");
			}
			for (JsonElement e : list("buffers")) {
				if (Geometry.object(e).has("uri")) {
					throw new IllegalArgumentException("only the file's own binary chunk may hold data (no external or data: URIs)");
				}
			}
			if (size("materials") > MAX_MATERIALS) {
				throw new IllegalArgumentException("more than " + MAX_MATERIALS + " materials");
			}
			if (size("images") > MAX_IMAGES) {
				throw new IllegalArgumentException("more than " + MAX_IMAGES + " textures");
			}
			List<byte[]> images = images();
			List<Material> materials = materials(images.size());
			List<Node> nodes = new ArrayList<Node>();
			Map<Integer, Integer> order = nodes(nodes);
			List<Primitive> primitives = primitives(order, materials.size());
			List<Channel> channels = new ArrayList<Channel>();
			float length = animation(order, channels);
			MeshModel mesh = new MeshModel(nodes, primitives, materials, images, channels, length, sheen());
			if (mesh.triangles() == 0) {
				throw new IllegalArgumentException("no triangles");
			}
			if (mesh.triangles() > MAX_TRIANGLES) {
				throw new IllegalArgumentException(mesh.triangles() + " triangles (at most " + MAX_TRIANGLES + ")");
			}
			return mesh;
		}

		int size(String key) {
			return list(key).size();
		}

		JsonArray list(String key) {
			JsonElement e = doc.get(key);
			return e != null && e.isJsonArray() ? e.getAsJsonArray() : new JsonArray();
		}

		static JsonArray list(JsonObject o, String key) {
			JsonElement e = o.get(key);
			return e != null && e.isJsonArray() ? e.getAsJsonArray() : new JsonArray();
		}

		int index(JsonElement e, String what, int len) {
			if (e == null || !e.isJsonPrimitive() || !e.getAsJsonPrimitive().isNumber()) {
				throw new IllegalArgumentException(what + " must be an index");
			}
			int i = e.getAsInt();
			if (i < 0 || i >= len) {
				throw new IllegalArgumentException(what + " " + i + " does not exist");
			}
			return i;
		}

		List<byte[]> images() {
			List<byte[]> out = new ArrayList<byte[]>();
			JsonArray views = list("bufferViews");
			int n = 0;
			for (JsonElement e : list("images")) {
				JsonObject img = Geometry.object(e);
				if (img.has("uri")) {
					throw new IllegalArgumentException("image " + n + " is an external file; embed it in the .glb");
				}
				if (!img.has("mimeType") || !"image/png".equals(img.get("mimeType").getAsString())) {
					throw new IllegalArgumentException("image " + n + " must be a PNG");
				}
				JsonObject view = Geometry.object(views.get(index(img.get("bufferView"), "bufferView", views.size())));
				int[] range = viewRange(view);
				byte[] png = Arrays.copyOfRange(raw, binAt + range[0], binAt + range[0] + range[1]);
				int[] side = Looks.pngSize(png);
				if (side == null || side[0] <= 0 || side[1] <= 0 || side[0] > MAX_TEXTURE_SIDE || side[1] > MAX_TEXTURE_SIDE) {
					throw new IllegalArgumentException("image " + n + " is not a PNG of at most " + MAX_TEXTURE_SIDE + "x" + MAX_TEXTURE_SIDE);
				}
				out.add(png);
				n++;
			}
			return out;
		}

		/** {offset, length} of a bufferView inside the binary chunk. */
		int[] viewRange(JsonObject view) {
			long off = view.has("byteOffset") ? view.get("byteOffset").getAsLong() : 0;
			if (!view.has("byteLength")) {
				throw new IllegalArgumentException("a bufferView has no length");
			}
			long len = view.get("byteLength").getAsLong();
			if (off < 0 || len < 0 || off + len > binLen) {
				throw new IllegalArgumentException("a bufferView runs past the binary chunk");
			}
			return new int[] {(int) off, (int) len};
		}

		List<Material> materials(int images) {
			JsonArray textures = list("textures");
			List<Material> out = new ArrayList<Material>();
			for (JsonElement e : list("materials")) {
				JsonObject m = Geometry.object(e);
				Material mat = new Material();
				JsonObject pbr = m.has("pbrMetallicRoughness") ? Geometry.object(m.get("pbrMetallicRoughness")) : null;
				if (pbr != null && pbr.has("baseColorFactor")) {
					mat.baseColor = floats(pbr.get("baseColorFactor"), 4, 0f, 1f);
				}
				if (pbr != null && pbr.has("baseColorTexture")) {
					mat.texture = texture(Geometry.object(pbr.get("baseColorTexture")), textures, images);
				}
				if (m.has("emissiveTexture")) {
					mat.emissiveTexture = texture(Geometry.object(m.get("emissiveTexture")), textures, images);
				}
				if (m.has("emissiveFactor")) {
					mat.emissiveFactor = floats(m.get("emissiveFactor"), 3, 0f, 1f);
				}
				String mode = m.has("alphaMode") ? m.get("alphaMode").getAsString() : "OPAQUE";
				if (mode.equals("MASK")) {
					mat.alpha = ALPHA_MASK;
				} else if (mode.equals("BLEND")) {
					mat.alpha = ALPHA_BLEND;
				} else if (!mode.equals("OPAQUE")) {
					throw new IllegalArgumentException("alphaMode " + mode);
				}
				mat.doubleSided = m.has("doubleSided") && m.get("doubleSided").getAsBoolean();
				out.add(mat);
			}
			return out;
		}

		int texture(JsonObject ref, JsonArray textures, int images) {
			if (ref.has("texCoord") && ref.get("texCoord").getAsInt() != 0) {
				throw new IllegalArgumentException("only TEXCOORD_0 is supported");
			}
			JsonObject t = Geometry.object(textures.get(index(ref.get("index"), "texture", textures.size())));
			return index(t.get("source"), "image", images);
		}

		float[] floats(JsonElement e, int n, float lo, float hi) {
			JsonArray a = Geometry.array(e);
			if (a.size() != n) {
				throw new IllegalArgumentException("wrong number of values");
			}
			float[] out = new float[n];
			for (int i = 0; i < n; i++) {
				out[i] = Math.max(lo, Math.min(hi, finite(a.get(i))));
			}
			return out;
		}

		float finite(JsonElement e) {
			if (e == null || !e.isJsonPrimitive() || !e.getAsJsonPrimitive().isNumber()) {
				throw new IllegalArgumentException("expected a number");
			}
			double d = e.getAsDouble();
			if (Double.isNaN(d) || Double.isInfinite(d) || Math.abs(d) > 1e9) {
				throw new IllegalArgumentException("number out of range");
			}
			return (float) d;
		}

		float[] trsField(JsonObject n, String key, float[] def) {
			if (!n.has(key)) {
				return def;
			}
			return floats(n.get(key), def.length, -1e9f, 1e9f);
		}

		/** The scene's {@code extras.arctic.sheen}, or null. */
		Sheen sheen() {
			JsonArray scenes = list("scenes");
			int scene = doc.has("scene") ? doc.get("scene").getAsInt() : 0;
			if (scene < 0 || scene >= scenes.size()) {
				return null;
			}
			JsonObject s = Geometry.object(scenes.get(scene));
			for (String key : new String[] {"extras", "arctic", "sheen"}) {
				if (!s.has(key) || !s.get(key).isJsonObject()) {
					return null;
				}
				s = s.getAsJsonObject(key);
			}
			JsonObject skew = s.has("skew") ? Geometry.object(s.get("skew")) : new JsonObject();
			float[] tint = s.has("tint") ? floats(s.get("tint"), 3, 0f, 1f) : new float[] {1, 1, 1};
			return new Sheen(number(s, "period", 3f, 0.5f, MAX_ANIMATION_SECS), tint, number(s, "strength", 1f, 0f, 1f),
					number(s, "width", 0.065f, 0.01f, 1f), s.has("axis") ? floats(s.get("axis"), 3, -1e3f, 1e3f) : new float[] {0, 1, 0},
					number(s, "origin", 0f, -1e3f, 1e3f), number(s, "length", 1f, 0.01f, 1e3f),
					skew.has("axis") ? floats(skew.get("axis"), 3, -1e3f, 1e3f) : new float[] {1, 0, 0}, number(skew, "amount", 0f, -1e3f, 1e3f),
					number(skew, "origin", 0f, -1e3f, 1e3f), skew.has("abs") && skew.get("abs").getAsBoolean());
		}

		float number(JsonObject o, String key, float def, float lo, float hi) {
			return o.has(key) ? Math.max(lo, Math.min(hi, finite(o.get(key)))) : def;
		}

		/** The scene's nodes, parents first; returns where each glTF node ended up. */
		Map<Integer, Integer> nodes(List<Node> out) {
			JsonArray raw = list("nodes");
			JsonArray scenes = list("scenes");
			int scene = doc.has("scene") ? doc.get("scene").getAsInt() : 0;
			if (scene < 0 || scene >= scenes.size()) {
				throw new IllegalArgumentException("the file has no scene");
			}
			JsonArray roots = list(Geometry.object(scenes.get(scene)), "nodes");
			Map<Integer, Integer> order = new HashMap<Integer, Integer>();
			List<int[]> stack = new ArrayList<int[]>();
			for (int i = roots.size() - 1; i >= 0; i--) {
				stack.add(new int[] {index(roots.get(i), "node", raw.size()), -1});
			}
			while (!stack.isEmpty()) {
				int[] top = stack.remove(stack.size() - 1);
				int i = top[0];
				if (order.containsKey(i)) {
					throw new IllegalArgumentException("node " + i + " is reached twice (nodes must form a tree)");
				}
				if (out.size() >= MAX_NODES) {
					throw new IllegalArgumentException("more than " + MAX_NODES + " nodes");
				}
				JsonObject n = Geometry.object(raw.get(i));
				for (String key : new String[] {"matrix", "skin", "camera", "weights"}) {
					if (n.has(key)) {
						throw new IllegalArgumentException("node " + i + ": " + key + " is not supported (use translation, rotation, scale)");
					}
				}
				float[] rotation = trsField(n, "rotation", new float[] {0, 0, 0, 1});
				float len = (float) Math.sqrt(rotation[0] * rotation[0] + rotation[1] * rotation[1] + rotation[2] * rotation[2] + rotation[3] * rotation[3]);
				if (len < 1e-6f) {
					throw new IllegalArgumentException("node " + i + ": rotation is zero");
				}
				for (int c = 0; c < 4; c++) {
					rotation[c] /= len;
				}
				float[] translation = trsField(n, "translation", new float[3]);
				float[] scale = trsField(n, "scale", new float[] {1, 1, 1});
				for (float v : translation) {
					if (Math.abs(v) > MAX_COORD) {
						throw new IllegalArgumentException("node " + i + ": translation or scale is too large");
					}
				}
				for (float v : scale) {
					if (Math.abs(v) > 64f) {
						throw new IllegalArgumentException("node " + i + ": translation or scale is too large");
					}
				}
				String name = n.has("name") && n.get("name").isJsonPrimitive() ? n.get("name").getAsString() : "";
				order.put(i, out.size());
				out.add(new Node(name.length() > 48 ? name.substring(0, 48) : name, top[1], translation, rotation, scale));
				int me = out.size() - 1;
				JsonArray kids = list(n, "children");
				for (int k = kids.size() - 1; k >= 0; k--) {
					stack.add(new int[] {index(kids.get(k), "node", raw.size()), me});
				}
			}
			return order;
		}

		List<Primitive> primitives(Map<Integer, Integer> order, int materials) {
			JsonArray meshes = list("meshes");
			JsonArray rawNodes = list("nodes");
			List<Primitive> out = new ArrayList<Primitive>();
			int triangles = 0;
			for (int g = 0; g < rawNodes.size(); g++) {
				JsonObject n = Geometry.object(rawNodes.get(g));
				Integer node = order.get(g);
				if (node == null || !n.has("mesh")) {
					continue;
				}
				JsonObject mesh = Geometry.object(meshes.get(index(n.get("mesh"), "mesh", meshes.size())));
				if (mesh.has("weights")) {
					throw new IllegalArgumentException("morph targets are not supported");
				}
				for (JsonElement pe : list(mesh, "primitives")) {
					if (out.size() >= MAX_PRIMITIVES) {
						throw new IllegalArgumentException("more than " + MAX_PRIMITIVES + " mesh primitives");
					}
					Primitive p = primitive(node, Geometry.object(pe), materials);
					triangles += p.indices.length / 3;
					if (triangles > MAX_TRIANGLES) {
						throw new IllegalArgumentException("more than " + MAX_TRIANGLES + " triangles");
					}
					out.add(p);
				}
			}
			return out;
		}

		Primitive primitive(int node, JsonObject prim, int materials) {
			if (prim.has("mode") && prim.get("mode").getAsInt() != 4) {
				throw new IllegalArgumentException("only triangle lists are supported");
			}
			if (prim.has("targets")) {
				throw new IllegalArgumentException("morph targets are not supported");
			}
			JsonObject attrs = Geometry.object(prim.get("attributes"));
			for (Map.Entry<String, JsonElement> e : attrs.entrySet()) {
				String k = e.getKey();
				if (!(k.equals("POSITION") || k.equals("NORMAL") || k.equals("TANGENT") || k.equals("TEXCOORD_0") || k.equals("TEXCOORD_1")
						|| k.equals("COLOR_0"))) {
					throw new IllegalArgumentException("attribute " + k + " is not supported");
				}
			}
			if (!attrs.has("POSITION")) {
				throw new IllegalArgumentException("a primitive has no POSITION");
			}
			float[] positions = vecs(attrs.get("POSITION").getAsInt(), 3, false);
			int n = positions.length / 3;
			for (float v : positions) {
				if (Math.abs(v) > MAX_COORD) {
					throw new IllegalArgumentException("a position is more than " + MAX_COORD + " pixels from the origin");
				}
			}
			float[] normals = attrs.has("NORMAL") ? vecs(attrs.get("NORMAL").getAsInt(), 3, false) : null;
			float[] uvs = attrs.has("TEXCOORD_0") ? vecs(attrs.get("TEXCOORD_0").getAsInt(), 2, true) : null;
			float[] colors = attrs.has("COLOR_0") ? colors(attrs.get("COLOR_0").getAsInt()) : null;
			if (normals != null && normals.length / 3 != n || uvs != null && uvs.length / 2 != n || colors != null && colors.length / 4 != n) {
				throw new IllegalArgumentException("a primitive's attributes have different lengths");
			}
			int[] indices;
			if (prim.has("indices")) {
				indices = indices(prim.get("indices").getAsInt());
			} else {
				indices = new int[n];
				for (int i = 0; i < n; i++) {
					indices[i] = i;
				}
			}
			if (indices.length == 0 || indices.length % 3 != 0) {
				throw new IllegalArgumentException("a primitive's index count is not a multiple of 3");
			}
			for (int i : indices) {
				if (i < 0 || i >= n) {
					throw new IllegalArgumentException("an index points past the vertices");
				}
			}
			int material = prim.has("material") ? index(prim.get("material"), "material", materials) : -1;
			if (normals == null) {
				// Flat shading: every triangle needs its own vertices.
				int m = indices.length;
				float[] pos = new float[m * 3];
				float[] nrm = new float[m * 3];
				float[] uv = uvs == null ? null : new float[m * 2];
				float[] col = colors == null ? null : new float[m * 4];
				for (int i = 0; i < m; i++) {
					int s = indices[i];
					System.arraycopy(positions, s * 3, pos, i * 3, 3);
					if (uv != null) {
						System.arraycopy(uvs, s * 2, uv, i * 2, 2);
					}
					if (col != null) {
						System.arraycopy(colors, s * 4, col, i * 4, 4);
					}
				}
				for (int t = 0; t < m; t += 3) {
					float[] f = faceNormal(pos, t * 3);
					for (int v = 0; v < 3; v++) {
						System.arraycopy(f, 0, nrm, (t + v) * 3, 3);
					}
				}
				positions = pos;
				normals = nrm;
				uvs = uv;
				colors = col;
				indices = new int[m];
				for (int i = 0; i < m; i++) {
					indices[i] = i;
				}
				n = m;
			} else {
				for (int i = 0; i < n; i++) {
					float x = normals[i * 3];
					float y = normals[i * 3 + 1];
					float z = normals[i * 3 + 2];
					float len = (float) Math.sqrt(x * x + y * y + z * z);
					if (len < 1e-6f) {
						normals[i * 3] = 0;
						normals[i * 3 + 1] = 1;
						normals[i * 3 + 2] = 0;
					} else {
						normals[i * 3] = x / len;
						normals[i * 3 + 1] = y / len;
						normals[i * 3 + 2] = z / len;
					}
				}
			}
			if (uvs == null) {
				uvs = new float[n * 2];
			}
			if (colors == null) {
				colors = new float[n * 4];
				Arrays.fill(colors, 1f);
			}
			return new Primitive(node, material, positions, normals, uvs, colors, indices);
		}

		static float[] faceNormal(float[] p, int at) {
			float ux = p[at + 3] - p[at];
			float uy = p[at + 4] - p[at + 1];
			float uz = p[at + 5] - p[at + 2];
			float vx = p[at + 6] - p[at];
			float vy = p[at + 7] - p[at + 1];
			float vz = p[at + 8] - p[at + 2];
			float nx = uy * vz - uz * vy;
			float ny = uz * vx - ux * vz;
			float nz = ux * vy - uy * vx;
			float len = (float) Math.sqrt(nx * nx + ny * ny + nz * nz);
			return len < 1e-12f ? new float[] {0, 1, 0} : new float[] {nx / len, ny / len, nz / len};
		}

		/** {view start, view length (in elements), element size, stride, component type, normalized}. */
		long[] accessor(int i, String kind, int... components) {
			JsonArray accessors = list("accessors");
			JsonArray views = list("bufferViews");
			if (i < 0 || i >= accessors.size()) {
				throw new IllegalArgumentException("an accessor does not exist");
			}
			JsonObject a = Geometry.object(accessors.get(i));
			if (a.has("sparse")) {
				throw new IllegalArgumentException("sparse accessors are not supported");
			}
			if (!a.has("type") || !kind.equals(a.get("type").getAsString())) {
				throw new IllegalArgumentException("an accessor must be " + kind);
			}
			int comp = a.has("componentType") ? a.get("componentType").getAsInt() : 0;
			boolean ok = false;
			for (int c : components) {
				ok |= c == comp;
			}
			if (!ok) {
				throw new IllegalArgumentException("accessor component type " + comp + " is not supported here");
			}
			long count = a.has("count") ? a.get("count").getAsLong() : 0;
			if (count <= 0 || count > MAX_ACCESSOR) {
				throw new IllegalArgumentException("an accessor has " + count + " elements (1 to " + MAX_ACCESSOR + ")");
			}
			JsonObject view = Geometry.object(views.get(index(a.get("bufferView"), "bufferView", views.size())));
			int[] range = viewRange(view);
			int ncomp = kind.equals("SCALAR") ? 1 : kind.equals("VEC2") ? 2 : kind.equals("VEC3") ? 3 : kind.equals("VEC4") ? 4 : 0;
			if (ncomp == 0) {
				throw new IllegalArgumentException("unsupported accessor type");
			}
			int csize = comp == 5121 ? 1 : comp == 5123 ? 2 : 4;
			int elem = ncomp * csize;
			long stride = view.has("byteStride") ? view.get("byteStride").getAsLong() : elem;
			if (stride < elem || stride > 252) {
				throw new IllegalArgumentException("bad byteStride");
			}
			long off = a.has("byteOffset") ? a.get("byteOffset").getAsLong() : 0;
			long end = off + stride * (count - 1) + elem;
			if (off < 0 || end > range[1]) {
				throw new IllegalArgumentException("an accessor runs past its bufferView");
			}
			boolean normalized = a.has("normalized") && a.get("normalized").getAsBoolean();
			return new long[] {range[0] + off, count, elem, stride, comp, normalized ? 1 : 0};
		}

		float[] vecs(int i, int n, boolean allowInt) {
			long[] a = accessor(i, "VEC" + n, allowInt ? new int[] {5126, 5121, 5123} : new int[] {5126});
			int comp = (int) a[4];
			if (comp != 5126 && a[5] == 0) {
				throw new IllegalArgumentException("integer attributes must be normalized");
			}
			int count = (int) a[1];
			float[] out = new float[count * n];
			for (int e = 0; e < count; e++) {
				int at = (int) (a[0] + e * a[3]);
				for (int c = 0; c < n; c++) {
					out[e * n + c] = component(at, c, comp);
				}
			}
			return out;
		}

		float component(int at, int c, int comp) {
			if (comp == 5126) {
				float v = bin.getFloat(at + c * 4);
				if (Float.isNaN(v) || Float.isInfinite(v)) {
					throw new IllegalArgumentException("a coordinate is not a finite number");
				}
				return v;
			}
			if (comp == 5121) {
				return (bin.get(at + c) & 0xFF) / 255f;
			}
			return (bin.getShort(at + c * 2) & 0xFFFF) / 65535f;
		}

		float[] colors(int i) {
			JsonArray accessors = list("accessors");
			if (i < 0 || i >= accessors.size()) {
				throw new IllegalArgumentException("an accessor does not exist");
			}
			String kind = Geometry.object(accessors.get(i)).get("type").getAsString();
			if (kind.equals("VEC4")) {
				return vecs(i, 4, true);
			}
			if (!kind.equals("VEC3")) {
				throw new IllegalArgumentException("COLOR_0 must be VEC3 or VEC4");
			}
			float[] rgb = vecs(i, 3, true);
			float[] out = new float[rgb.length / 3 * 4];
			for (int v = 0; v < rgb.length / 3; v++) {
				out[v * 4] = rgb[v * 3];
				out[v * 4 + 1] = rgb[v * 3 + 1];
				out[v * 4 + 2] = rgb[v * 3 + 2];
				out[v * 4 + 3] = 1f;
			}
			return out;
		}

		int[] indices(int i) {
			long[] a = accessor(i, "SCALAR", 5121, 5123, 5125);
			int count = (int) a[1];
			int[] out = new int[count];
			for (int e = 0; e < count; e++) {
				int at = (int) (a[0] + e * a[3]);
				out[e] = a[4] == 5121 ? bin.get(at) & 0xFF : a[4] == 5123 ? bin.getShort(at) & 0xFFFF : bin.getInt(at);
			}
			return out;
		}

		float[] scalars(int i) {
			long[] a = accessor(i, "SCALAR", 5126);
			int count = (int) a[1];
			float[] out = new float[count];
			for (int e = 0; e < count; e++) {
				float v = bin.getFloat((int) (a[0] + e * a[3]));
				if (Float.isNaN(v) || Float.isInfinite(v)) {
					throw new IllegalArgumentException("an animation time is not a finite number");
				}
				out[e] = v;
			}
			return out;
		}

		/** Fills {@code out}; returns the animation's length (0 when there isn't one). */
		float animation(Map<Integer, Integer> order, List<Channel> out) {
			JsonArray list = list("animations");
			if (list.size() > 1) {
				throw new IllegalArgumentException("only one animation is supported");
			}
			if (list.size() == 0) {
				return 0f;
			}
			JsonObject a = Geometry.object(list.get(0));
			JsonArray samplers = list(a, "samplers");
			JsonArray accessors = list("accessors");
			float length = 0f;
			for (JsonElement ce : list(a, "channels")) {
				if (out.size() >= MAX_CHANNELS) {
					throw new IllegalArgumentException("more than " + MAX_CHANNELS + " animation channels");
				}
				JsonObject c = Geometry.object(ce);
				JsonObject target = Geometry.object(c.get("target"));
				int raw = index(target.get("node"), "node", list("nodes").size());
				Integer node = order.get(raw);
				if (node == null) {
					throw new IllegalArgumentException("an animation moves a node that is not in the scene");
				}
				String pathName = target.has("path") ? target.get("path").getAsString() : "";
				int path;
				if (pathName.equals("translation")) {
					path = TRANSLATION;
				} else if (pathName.equals("rotation")) {
					path = ROTATION;
				} else if (pathName.equals("scale")) {
					path = SCALE;
				} else {
					throw new IllegalArgumentException("animation path " + pathName + " is not supported");
				}
				JsonObject s = Geometry.object(samplers.get(index(c.get("sampler"), "sampler", samplers.size())));
				String interpolation = s.has("interpolation") ? s.get("interpolation").getAsString() : "LINEAR";
				if (!interpolation.equals("LINEAR") && !interpolation.equals("STEP")) {
					throw new IllegalArgumentException(interpolation + " interpolation is not supported (use LINEAR or STEP)");
				}
				float[] times = scalars(index(s.get("input"), "accessor", accessors.size()));
				if (times.length > MAX_KEYFRAMES) {
					throw new IllegalArgumentException("animation times must be increasing, finite, and at most 256 keys");
				}
				for (int k = 0; k < times.length; k++) {
					if (times[k] < 0 || k > 0 && times[k] < times[k - 1]) {
						throw new IllegalArgumentException("animation times must be increasing, finite, and at most 256 keys");
					}
				}
				int outAcc = index(s.get("output"), "accessor", accessors.size());
				float[] values;
				if (path == ROTATION) {
					values = vecs(outAcc, 4, false);
					for (int k = 0; k < values.length; k += 4) {
						float len = (float) Math.sqrt(values[k] * values[k] + values[k + 1] * values[k + 1] + values[k + 2] * values[k + 2]
								+ values[k + 3] * values[k + 3]);
						if (len < 1e-6f) {
							values[k] = 0;
							values[k + 1] = 0;
							values[k + 2] = 0;
							values[k + 3] = 1;
						} else {
							for (int q = 0; q < 4; q++) {
								values[k + q] /= len;
							}
						}
					}
				} else {
					float[] v3 = vecs(outAcc, 3, false);
					values = new float[v3.length / 3 * 4];
					for (int k = 0; k < v3.length / 3; k++) {
						for (int q = 0; q < 3; q++) {
							float v = v3[k * 3 + q];
							if (Math.abs(v) > MAX_COORD) {
								throw new IllegalArgumentException("an animated translation or scale is too large");
							}
							values[k * 4 + q] = v;
						}
					}
				}
				if (values.length / 4 != times.length) {
					throw new IllegalArgumentException("an animation sampler's input and output differ in length");
				}
				length = Math.max(length, times[times.length - 1]);
				out.add(new Channel(node, path, interpolation.equals("STEP"), times, values));
			}
			if (out.isEmpty()) {
				return 0f;
			}
			if (length <= 0 || length > MAX_ANIMATION_SECS) {
				throw new IllegalArgumentException("the animation must last 0 to " + MAX_ANIMATION_SECS + " seconds");
			}
			return length;
		}
	}
}
