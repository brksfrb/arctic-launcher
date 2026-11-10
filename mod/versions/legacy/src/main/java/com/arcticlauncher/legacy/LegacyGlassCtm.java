package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.ctm.GlassTiles;
import java.util.AbstractList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.RandomAccess;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.MinecraftClient;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import net.minecraft.block.Block;
import net.minecraft.block.BlockState;
import net.minecraft.world.BlockView;
import net.minecraft.client.render.model.BakedQuad;
import net.minecraft.client.texture.Sprite;

/** Terrain-only CTM context. Each meshing thread owns its scratch positions and neighbour cache. */
public final class LegacyGlassCtm {
	public static volatile com.arcticlauncher.client.ctm.GlassPacks packs = com.arcticlauncher.client.ctm.GlassPacks.EMPTY;
	private static final boolean CONTINUITY = FabricLoader.getInstance().isModLoaded("continuity");
	private static volatile Map<Sprite, Sprite[]> sprites = new IdentityHashMap<>();
	private static boolean lastEnabled = true;
	private static final ThreadLocal<Context> LOCAL = ThreadLocal.withInitial(Context::new);
	private LegacyGlassCtm() {}

	public static boolean available() { return !CONTINUITY; }
	public static boolean enabled() { return available() && (ArcticClient.config() == null || ArcticClient.config().connectedGlass); }

	public static void tick() {
		boolean on = enabled();
		if (lastEnabled != on) {
			lastEnabled = on;
			if (MinecraftClient.getInstance().world == null) return;
			MinecraftClient.getInstance().worldRenderer.reload();
		}
	}

	public static void bind(Map<?, Sprite> atlas) {
		Map<String, Sprite> names = new java.util.HashMap<>();
		for (Map.Entry<?, Sprite> e : atlas.entrySet()) names.put(e.getKey().toString(), e.getValue());
		Map<Sprite, Sprite[]> found = new IdentityHashMap<>();
		for (Map.Entry<String, Sprite> e : names.entrySet()) {
			if (GlassTiles.paneEdge(e.getKey())) { found.put(e.getValue(), new Sprite[]{e.getValue()}); continue; }
			if (!packs.acceptsSprite(e.getKey())) continue;
			Sprite[] variants = new Sprite[GlassTiles.VARIANTS];
			boolean complete = true;
			for (int i = 0; i < variants.length; i++) {
				variants[i] = names.get(GlassTiles.variant(e.getKey(), i));
				complete &= variants[i] != null;
			}
			if (complete) found.put(e.getValue(), variants);
		}
		sprites = found;
	}

	public static void begin(BlockView world, BlockState block, BlockPos pos) {
		Context c = LOCAL.get();
		c.active = false;
		if (!enabled()) return;
		Integer type = c.blocks.get(block.getBlock());
		if (type == null) {
			String id = Block.REGISTRY.getIdentifier(block.getBlock()).toString();
			boolean glass = id.equals("minecraft:stained_glass") || id.equals("minecraft:stained_glass_pane") || id.equals("minecraft:glass_pane") || id.endsWith("_stained_glass_pane") || GlassTiles.glass(id);
			type = glass ? (id.endsWith("pane") ? 2 : 1) : 0;
			c.blocks.put(block.getBlock(), type);
		}
		if (type == 0) return;
		c.pane = type == 2;
		c.world = world; c.block = block.getBlock(); c.data = c.block.getData(block);
		c.bank = GlassTiles.bank(Block.REGISTRY.getIdentifier(c.block).toString(), c.data);
		c.x = pos.getX(); c.y = pos.getY(); c.z = pos.getZ();
		c.checked = 0; c.connected = 0; c.faces = 0; c.viewIndex = 0; c.active = true;
	}

	public static void end() { Context c = LOCAL.get(); c.active = false; c.world = null; }

	public static List<BakedQuad> quads(List<BakedQuad> original) {
		Context c = LOCAL.get();
		if (!c.active || original.isEmpty() || original.size() > 128) return original;
		// Consumers finish each face list before asking for the next. Separate slots also cover
		// multipart models that collect several face lists before emitting their vertices.
		if (c.viewIndex == c.views.length) c.viewIndex = 0;
		QuadList list = c.views[c.viewIndex++];
		list.source = original;
		list.count = 0;
		for (int i = 0; i < original.size(); i++) {
			Variants variants = ((GlassQuad) (Object) original.get(i)).arctic$ctm();
			if (!c.hide(variants)) list.indices[list.count++] = i;
		}
		return list;
	}

	private static final class Context {
		boolean active, pane;
		int bank;
		int data, x, y, z, checked, connected, faces, viewIndex;
		Block block;
		BlockView world;
		final BlockPos.Mutable scratch = new BlockPos.Mutable();
		final int[] masks = new int[6];
		final IdentityHashMap<Block, Integer> blocks = new IdentityHashMap<>();
		final QuadList[] views = new QuadList[32];
		Context() { for (int i = 0; i < views.length; i++) views[i] = new QuadList(this); }
		boolean same(int dx, int dy, int dz) {
			int bit = 1 << ((dy + 1) * 9 + (dz + 1) * 3 + dx + 1);
			if ((checked & bit) == 0) {
				scratch.setPosition(x + dx, y + dy, z + dz);
				BlockState other = world.getBlockState(scratch);
				if (other.getBlock() == block && block.getData(other) == data) connected |= bit;
				checked |= bit;
			}
			return (connected & bit) != 0;
		}
		boolean hide(Variants variants) {
			if (!pane || variants == null || !variants.edge || !variants.boundary) return false;
			Direction face = variants.direction;
			return same(face == Direction.EAST ? 1 : face == Direction.WEST ? -1 : 0,
					face == Direction.UP ? 1 : face == Direction.DOWN ? -1 : 0,
					face == Direction.SOUTH ? 1 : face == Direction.NORTH ? -1 : 0);
		}
		int mask(Direction face) {
			int index = face.ordinal();
			if ((faces & (1 << index)) != 0) return masks[index];
			// Face-local right and bottom in vanilla's UV orientation.
			int rx = 0, rz = 0, by = -1, bz = 0;
			switch (face) {
				case NORTH: rx = -1; break;
				case SOUTH: rx = 1; break;
				case WEST: rz = 1; break;
				case EAST: rz = -1; break;
				case UP: rx = 1; by = 0; bz = 1; break;
				case DOWN: rx = 1; by = 0; bz = -1; break;
			}
			int mask = 0;
			if (same(-rx, 0, -rz)) mask |= 1;
			if (same(0, by, bz)) mask |= 4;
			if (same(rx, 0, rz)) mask |= 16;
			if (same(0, -by, -bz)) mask |= 64;
			if ((mask & 5) == 5 && same(-rx, by, -rz + bz)) mask |= 2;
			if ((mask & 20) == 20 && same(rx, by, rz + bz)) mask |= 8;
			if ((mask & 80) == 80 && same(rx, -by, rz - bz)) mask |= 32;
			if ((mask & 65) == 65 && same(-rx, -by, -rz - bz)) mask |= 128;
			faces |= 1 << index;
			return masks[index] = mask;
		}
	}

	private static final class QuadList extends AbstractList<BakedQuad> implements RandomAccess {
		final Context context;
		List<BakedQuad> source;
		int count;
		final int[] indices = new int[128];
		QuadList(Context context) { this.context = context; }
		@Override public int size() { return count; }
		@Override public BakedQuad get(int index) {
			BakedQuad quad = source.get(indices[index]);
			Variants variants = ((GlassQuad) (Object) quad).arctic$ctm();
			if (variants == null || variants.edge) return quad;
			int tile = GlassTiles.index(GlassTiles.orient(context.mask(variants.direction), variants.orientation));
			return variants.quads[tile + context.bank * GlassTiles.COUNT];
		}
	}

	private static Sprite spriteOf(BakedQuad quad) {
		//#if MC >= 1.9
		return quad.method_12351();
		//#else
		int[] data = quad.getVertexData();
		int stride = data.length / 4;
		float u = 0, v = 0;
		for (int i = 0; i < 4; i++) {
			u += Float.intBitsToFloat(data[i * stride + 4]) * 0.25f;
			v += Float.intBitsToFloat(data[i * stride + 5]) * 0.25f;
		}
		for (Sprite sprite : sprites.keySet()) {
			if (u >= sprite.getMinU() && u <= sprite.getMaxU() && v >= sprite.getMinV() && v <= sprite.getMaxV()) return sprite;
		}
		return null;
		//#endif
	}

	/** Called while models bake, after their atlas is stitched. Selection never allocates. */
	public static Variants prepare(BakedQuad quad) {
		if (!available() || sprites.isEmpty()) return null;
		Sprite sprite = spriteOf(quad);
		Sprite[] tiles = sprites.get(sprite);
		return tiles == null ? null : new Variants(quad, sprite, tiles);
	}

	public static final class Variants {
		final BakedQuad[] quads;
		final boolean edge, boundary;
		final Direction direction;
		final int orientation;
		Variants(BakedQuad quad, Sprite sprite, Sprite[] tiles) {
			direction = quad.getFace();
			edge = tiles.length == 1;
			int axis = direction == Direction.UP || direction == Direction.DOWN ? 1 : direction == Direction.NORTH || direction == Direction.SOUTH ? 2 : 0;
			float plane = coordinate(quad, 0, axis);
			boundary = Math.abs(plane) < 0.00001f || Math.abs(plane - 1) < 0.00001f;
			orientation = orientation(quad, direction);
			if (edge) { quads = new BakedQuad[]{quad}; return; }
			quads = new BakedQuad[GlassTiles.VARIANTS];
			for (int i = 0; i < quads.length; i++) quads[i] = remap(quad, sprite, tiles[i]);
		}
	}

	private static int orientation(BakedQuad quad, Direction face) {
		int u = 2, v = 1;
		for (int i = 0; i < 4; i++) {
			int j = (i + 1) & 3;
			float du = coordinate(quad, j, 3) - coordinate(quad, i, 3);
			float dv = coordinate(quad, j, 4) - coordinate(quad, i, 4);
			float dx = coordinate(quad, j, 0) - coordinate(quad, i, 0);
			float dy = coordinate(quad, j, 1) - coordinate(quad, i, 1);
			float dz = coordinate(quad, j, 2) - coordinate(quad, i, 2);
			float right = face == Direction.NORTH ? -dx : face == Direction.WEST ? dz : face == Direction.EAST ? -dz : dx;
			float bottom = face == Direction.UP ? dz : face == Direction.DOWN ? -dz : -dy;
			if (Math.abs(du) > Math.abs(dv) * 100 + 0.000001f) u = axis(right / du, bottom / du);
			if (Math.abs(dv) > Math.abs(du) * 100 + 0.000001f) v = axis(right / dv, bottom / dv);
		}
		return u | (v << 2);
	}
	private static int axis(float right, float bottom) {
		return Math.abs(right) > Math.abs(bottom) ? (right > 0 ? 2 : 0) : (bottom > 0 ? 1 : 3);
	}
	private static float coordinate(BakedQuad quad, int vertex, int coordinate) {
		int[] data = quad.getVertexData();
		return Float.intBitsToFloat(data[vertex * (data.length / 4) + (coordinate < 3 ? coordinate : coordinate + 1)]);
	}

	private static float u(float value, Sprite from, Sprite to) {
		return to.getMinU() + (value - from.getMinU()) * (to.getMaxU() - to.getMinU()) / (from.getMaxU() - from.getMinU());
	}
	private static float v(float value, Sprite from, Sprite to) {
		return to.getMinV() + (value - from.getMinV()) * (to.getMaxV() - to.getMinV()) / (from.getMaxV() - from.getMinV());
	}
	private static BakedQuad remap(BakedQuad quad, Sprite from, Sprite to) {
		int[] data = quad.getVertexData().clone();
		int stride = data.length / 4;
		for (int i = 0; i < 4; i++) {
			data[i * stride + 4] = Float.floatToRawIntBits(u(Float.intBitsToFloat(data[i * stride + 4]), from, to));
			data[i * stride + 5] = Float.floatToRawIntBits(v(Float.intBitsToFloat(data[i * stride + 5]), from, to));
		}
		//#if MC >= 1.9
		return new BakedQuad(data, quad.getColorIndex(), quad.getFace(), to);
		//#else
		return new BakedQuad(data, quad.getColorIndex(), quad.getFace());
		//#endif
	}
}
