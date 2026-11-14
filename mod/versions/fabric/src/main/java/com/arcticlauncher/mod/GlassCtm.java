package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.ctm.GlassTiles;
import java.util.AbstractList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.RandomAccess;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
//#if MC >= 26.1
import net.minecraft.client.renderer.block.BlockAndTintGetter;
import net.minecraft.client.resources.model.geometry.BakedQuad;
//#elif MC >= 1.15
import net.minecraft.world.level.BlockAndTintGetter;
import net.minecraft.client.renderer.block.model.BakedQuad;
//#else
import net.minecraft.world.level.BlockAndBiomeGetter;
import net.minecraft.client.renderer.block.model.BakedQuad;
//#endif
import net.minecraft.client.renderer.texture.TextureAtlasSprite;

/** Terrain-only CTM context. Each meshing thread owns its scratch positions and neighbour cache. */
public final class GlassCtm {
	private static final boolean CONTINUITY = FabricLoader.getInstance().isModLoaded("continuity");
	private static volatile Map<TextureAtlasSprite, TextureAtlasSprite[]> sprites = new IdentityHashMap<>();
	private static boolean lastEnabled = true;
	private static final ThreadLocal<Context> LOCAL = ThreadLocal.withInitial(Context::new);
	private GlassCtm() {}

	public static boolean available() { return !CONTINUITY; }
	public static boolean enabled() { return available() && (ArcticClient.config() == null || ArcticClient.config().connectedGlass); }

	public static void tick() {
		// The line level (clarity of the glass texture) is part of the tiles: stitching the atlas again also rebuilds every chunk.
		if (available() && com.arcticlauncher.client.ctm.GlassLines.due(available())) {
			lastEnabled = enabled();
			//#if MC >= 1.14
			Minecraft.getInstance().reloadResourcePacks();
			//#else
			Minecraft.getInstance().reloadResources();
			//#endif
			return;
		}
		boolean on = enabled();
		if (lastEnabled != on) {
			lastEnabled = on;
			if (Minecraft.getInstance().level == null) return;
			//#if MC >= 26.2
			Minecraft.getInstance().levelExtractor.allChanged();
			//#else
			Minecraft.getInstance().levelRenderer.allChanged();
			//#endif
		}
	}

	public static void bind(Map<?, TextureAtlasSprite> atlas) {
		Map<String, TextureAtlasSprite> names = new java.util.HashMap<>();
		for (Map.Entry<?, TextureAtlasSprite> e : atlas.entrySet()) names.put(e.getKey().toString(), e.getValue());
		Map<TextureAtlasSprite, TextureAtlasSprite[]> found = new IdentityHashMap<>();
		for (Map.Entry<String, TextureAtlasSprite> e : names.entrySet()) {
			if (GlassTiles.paneEdge(e.getKey())) { found.put(e.getValue(), new TextureAtlasSprite[]{e.getValue()}); continue; }
			if (!GlassTiles.glass(e.getKey())) continue;
			TextureAtlasSprite[] variants = new TextureAtlasSprite[GlassTiles.COUNT];
			boolean complete = true;
			for (int i = 0; i < variants.length; i++) {
				variants[i] = names.get(GlassTiles.variant(e.getKey(), i));
				complete &= variants[i] != null;
			}
			if (complete) found.put(e.getValue(), variants);
		}
		sprites = found;
	}

	//#if MC >= 1.15
	public static void begin(BlockAndTintGetter world, BlockState block, BlockPos pos) {
	//#else
	public static void begin(BlockAndBiomeGetter world, BlockState block, BlockPos pos) {
	//#endif
		Context c = LOCAL.get();
		c.active = false;
		// With Connected Glass off the glass still takes our texture (lone tile, clarity applied), it just joins to nothing.
		if (c.damageDepth != 0 || !available()) return;
		c.joined = enabled();
		Integer type = c.blocks.get(block.getBlock());
		if (type == null) {
			//#if MC >= 1.19.3
			String id = net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(block.getBlock()).toString();
			//#elif MC >= 1.14
			String id = net.minecraft.core.Registry.BLOCK.getKey(block.getBlock()).toString();
			//#else
			String id = net.minecraft.core.Registry.BLOCK.getKey(block.getBlock()).toString();
			//#endif
			boolean glass = id.equals("minecraft:glass_pane") || id.endsWith("_stained_glass_pane") || GlassTiles.glass(id);
			type = glass ? (id.endsWith("pane") ? 2 : 1) : 0;
			c.blocks.put(block.getBlock(), type);
		}
		if (type == 0) return;
		c.pane = type == 2;
		c.world = world; c.block = block.getBlock();
		c.x = pos.getX(); c.y = pos.getY(); c.z = pos.getZ();
		c.checked = 0; c.connected = 0; c.faces = 0; c.viewIndex = 0; c.active = true;
		//#if DEV_SCENES
		//#if MC >= 1.21.11 && MC < 26.1
		if (GlassBordersTest.watching(pos)) GlassBordersTest.observe(pos, c.mask(Direction.DOWN) << 8 | c.mask(Direction.SOUTH));
		GlassFramesTest.meshing(c.x, c.y, c.z);
		//#endif
		//#endif
	}

	/** Damage rendering shares the terrain renderer, but must retain vanilla crack textures. */
	public static void beginDamage() { LOCAL.get().damageDepth++; end(); }
	public static void endDamage() { LOCAL.get().damageDepth--; end(); }
	//#if DEV_SCENES
	//#if MC >= 1.21.11 && MC < 26.1
	/** The masks a block gets from a view: down << 8 | south. */
	static int masks(BlockAndTintGetter world, BlockState block, BlockPos pos) {
		begin(world, block, pos);
		Context c = LOCAL.get();
		int masks = c.active ? c.mask(Direction.DOWN) << 8 | c.mask(Direction.SOUTH) : -1;
		end();
		return masks;
	}
	//#endif
	//#endif

	//#if DEV_SCENES
	//#if MC >= 1.21.11 && MC < 26.1
	/** The mask one face of a block gets from a view; -1 when Connected Glass leaves the block alone. */
	static int mask(BlockAndTintGetter world, BlockState block, BlockPos pos, Direction face) {
		begin(world, block, pos);
		Context c = LOCAL.get();
		int mask = c.active ? c.mask(face) : -1;
		end();
		return mask;
	}
	//#endif
	//#endif

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
		boolean active, pane, joined;
		int damageDepth;
		int x, y, z, checked, connected, faces, viewIndex;
		Block block;
		//#if MC >= 1.15
		BlockAndTintGetter world;
		//#else
		BlockAndBiomeGetter world;
		//#endif
		final BlockPos.MutableBlockPos scratch = new BlockPos.MutableBlockPos();
		final int[] masks = new int[6];
		final IdentityHashMap<Block, Integer> blocks = new IdentityHashMap<>();
		final QuadList[] views = new QuadList[32];
		Context() { for (int i = 0; i < views.length; i++) views[i] = new QuadList(this); }
		boolean same(int dx, int dy, int dz) {
			int bit = 1 << ((dy + 1) * 9 + (dz + 1) * 3 + dx + 1);
			if ((checked & bit) == 0) {
				scratch.set(x + dx, y + dy, z + dz);
				if (world.getBlockState(scratch).getBlock() == block) connected |= bit;
				checked |= bit;
			}
			return (connected & bit) != 0;
		}
		boolean hide(Variants variants) {
			if (!joined || !pane || variants == null || !variants.edge || !variants.boundary) return false;
			Direction face = variants.direction;
			return same(face == Direction.EAST ? 1 : face == Direction.WEST ? -1 : 0,
					face == Direction.UP ? 1 : face == Direction.DOWN ? -1 : 0,
					face == Direction.SOUTH ? 1 : face == Direction.NORTH ? -1 : 0);
		}
		int mask(Direction face) {
			int index = face.ordinal();
			if ((faces & (1 << index)) != 0) return masks[index];
			if (!joined) { faces |= 1 << index; return masks[index] = 0; }
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
			//#if DEV_SCENES
			//#if MC >= 1.21.11 && MC < 26.1
			GlassFramesTest.observe(x, y, z, index, mask);
			//#endif
			//#endif
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
			return variants.quads[tile];
		}
	}

	/** Called while models bake, after their atlas is stitched. Selection never allocates. */
	public static Variants prepare(BakedQuad quad) {
		// Called from the base constructor: subclass getters may read uninitialized
		// fields, and damage overlays have not replaced their UVs yet.
		if (quad.getClass() != BakedQuad.class) return null;
		if (!available() || sprites.isEmpty()) return null;
		//#if MC >= 26.1
		TextureAtlasSprite sprite = quad.materialInfo().sprite();
		//#elif MC >= 1.21.5
		TextureAtlasSprite sprite = quad.sprite();
		//#else
		TextureAtlasSprite sprite = ((com.arcticlauncher.mod.mixin.GlassQuadAccess) (Object) quad).arctic$sprite();
		//#endif
		TextureAtlasSprite[] tiles = sprites.get(sprite);
		return tiles == null ? null : new Variants(quad, sprite, tiles);
	}

	public static final class Variants {
		final BakedQuad[] quads;
		final boolean edge, boundary;
		final Direction direction;
		final int orientation;
		Variants(BakedQuad quad, TextureAtlasSprite sprite, TextureAtlasSprite[] tiles) {
			//#if MC >= 1.21.5
			direction = quad.direction();
			//#else
			direction = quad.getDirection();
			//#endif
			edge = tiles.length == 1;
			int axis = direction == Direction.UP || direction == Direction.DOWN ? 1 : direction == Direction.NORTH || direction == Direction.SOUTH ? 2 : 0;
			float plane = coordinate(quad, 0, axis);
			boundary = Math.abs(plane) < 0.00001f || Math.abs(plane - 1) < 0.00001f;
			orientation = orientation(quad, direction);
			if (edge) { quads = new BakedQuad[]{quad}; return; }
			quads = new BakedQuad[GlassTiles.COUNT];
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
		//#if MC >= 1.21.11
		if (coordinate < 3) {
			org.joml.Vector3fc p = quad.position(vertex);
			return coordinate == 0 ? p.x() : coordinate == 1 ? p.y() : p.z();
		}
		return coordinate == 3 ? net.minecraft.client.model.geom.builders.UVPair.unpackU(quad.packedUV(vertex)) : net.minecraft.client.model.geom.builders.UVPair.unpackV(quad.packedUV(vertex));
		//#else
		//#if MC >= 1.21.5
		int[] data = quad.vertices();
		//#else
		int[] data = quad.getVertices();
		//#endif
		return Float.intBitsToFloat(data[vertex * (data.length / 4) + (coordinate < 3 ? coordinate : coordinate + 1)]);
		//#endif
	}

	private static float u(float value, TextureAtlasSprite from, TextureAtlasSprite to) {
		return to.getU0() + (value - from.getU0()) * (to.getU1() - to.getU0()) / (from.getU1() - from.getU0());
	}
	private static float v(float value, TextureAtlasSprite from, TextureAtlasSprite to) {
		return to.getV0() + (value - from.getV0()) * (to.getV1() - to.getV0()) / (from.getV1() - from.getV0());
	}
	//#if MC >= 1.21.11
	private static long uv(long packed, TextureAtlasSprite from, TextureAtlasSprite to) {
		return net.minecraft.client.model.geom.builders.UVPair.pack(
				u(net.minecraft.client.model.geom.builders.UVPair.unpackU(packed), from, to),
				v(net.minecraft.client.model.geom.builders.UVPair.unpackV(packed), from, to));
	}
	//#endif
	private static BakedQuad remap(BakedQuad quad, TextureAtlasSprite from, TextureAtlasSprite to) {
		//#if MC >= 26.1
		BakedQuad.MaterialInfo m = quad.materialInfo();
		//#if MC >= 26.3
		BakedQuad.MaterialInfo material = new BakedQuad.MaterialInfo(to, m.layer(), m.itemRenderType(), m.itemGlintRenderType(), m.itemGlintSpecialRenderType(), m.tintIndex(), m.shadeDirectionOverride(), m.lightEmission());
		//#else
		BakedQuad.MaterialInfo material = new BakedQuad.MaterialInfo(to, m.layer(), m.itemRenderType(), m.tintIndex(), m.shade(), m.lightEmission());
		//#endif
		return new BakedQuad(quad.position0(), quad.position1(), quad.position2(), quad.position3(), uv(quad.packedUV0(), from, to), uv(quad.packedUV1(), from, to), uv(quad.packedUV2(), from, to), uv(quad.packedUV3(), from, to), quad.direction(), material);
		//#elif MC >= 1.21.11
		return new BakedQuad(quad.position0(), quad.position1(), quad.position2(), quad.position3(), uv(quad.packedUV0(), from, to), uv(quad.packedUV1(), from, to), uv(quad.packedUV2(), from, to), uv(quad.packedUV3(), from, to), quad.tintIndex(), quad.direction(), to, quad.shade(), quad.lightEmission());
		//#else
		//#if MC >= 1.21.5
		int[] data = quad.vertices().clone();
		//#else
		int[] data = quad.getVertices().clone();
		//#endif
		int stride = data.length / 4;
		for (int i = 0; i < 4; i++) {
			data[i * stride + 4] = Float.floatToRawIntBits(u(Float.intBitsToFloat(data[i * stride + 4]), from, to));
			data[i * stride + 5] = Float.floatToRawIntBits(v(Float.intBitsToFloat(data[i * stride + 5]), from, to));
		}
		//#if MC >= 1.21.5
		return new BakedQuad(data, quad.tintIndex(), quad.direction(), to, quad.shade(), quad.lightEmission());
		//#elif MC >= 1.21.2
		return new BakedQuad(data, quad.getTintIndex(), quad.getDirection(), to, quad.isShade(), quad.getLightEmission());
		//#elif MC >= 1.16
		return new BakedQuad(data, quad.getTintIndex(), quad.getDirection(), to, quad.isShade());
		//#else
		return new BakedQuad(data, quad.getTintIndex(), quad.getDirection(), to);
		//#endif
		//#endif
	}
}
