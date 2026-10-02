//#if MC >= 26.2
package com.arcticlauncher.polonium.gpu;

import com.mojang.blaze3d.GpuFormat;
import com.mojang.blaze3d.systems.GpuDevice;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.textures.GpuTexture;
import com.mojang.blaze3d.textures.GpuTextureView;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import org.jspecify.annotations.Nullable;

/**
 * Every 64×64 entity texture in use (player skins, mostly) side by side in one
 * texture, so entities that differ only in their skin share one draw. A
 * server full of players has nearly a skin per player; drawn one texture at
 * a time, that's a draw per player.
 *
 * It's one texture, not several pages: translucent players are drawn back to
 * front, so only neighbors in that order can share a draw, and with pages
 * they'd keep switching. It starts small (2048², 16 MB) and doubles as more
 * skins are in use at once, up to what the GPU allows (8192², about 15,000
 * skins); skins keep their place when it grows. Past that, cells of skins not
 * drawn for a while are handed to new ones.
 *
 * A texture is copied in the first time it's drawn, as it is, with a
 * one-pixel border repeating its edge (so sampling at a skin's edge reads
 * that skin), and sampled with its own settings: pixels come out the same.
 * Render thread only.
 */
final class SkinAtlas {
	static final int CELL = 64;
	/** A cell with its border. */
	private static final int STRIDE = CELL + 2;
	private static final int FIRST_SIZE = 2048;
	private static final int MAX_SIZE = 8192;
	/** A cell may go to another texture once its own hasn't been drawn for this many frames. */
	private static final long IDLE_FRAMES = 600;
	/** Looking for a free cell stops after this many tries in a frame (the rest wait for the next frame). */
	private static final int SCAN_BUDGET = 256;
	/** Off with -Dpolonium.skinAtlas=false: each texture keeps its own draws. */
	static final boolean ENABLED = !"false".equals(System.getProperty("polonium.skinAtlas"));

	private @Nullable GpuTexture texture;
	private @Nullable GpuTextureView view;
	private int size;
	/** Replaced by a bigger one this frame: closed once the frame is drawn (draws already set up may use it). */
	private final List<GpuTexture> retired = new ArrayList<>();
	private final List<GpuTextureView> retiredViews = new ArrayList<>();

	private final Map<GpuTexture, Integer> cells = new IdentityHashMap<>();
	/** Per cell: its skin's top-left pixel (cells keep their place as the atlas grows), its texture and when it was drawn. */
	private int[] cellX = new int[0];
	private int[] cellY = new int[0];
	private GpuTexture[] owners = new GpuTexture[0];
	private long[] lastUsed = new long[0];
	private int nextFree;
	private int scan;
	/** No free cell was found this frame: don't look again until the next. */
	private long fullAt = -1;
	private boolean failed;

	/** The atlas, for drawing. */
	@Nullable GpuTextureView view() {
		return view;
	}

	/** The atlas's width and height, in pixels. */
	int size() {
		return size;
	}

	/** Left edge (in pixels) of a cell's skin. */
	int cellX(int cell) {
		return cellX[cell];
	}

	/** Top edge (in pixels) of a cell's skin. */
	int cellY(int cell) {
		return cellY[cell];
	}

	/** Whether this texture can go in the atlas (64×64, one mip level, plain RGBA, may be copied from). */
	static boolean fits(GpuTextureView source) {
		GpuTexture t = source.texture();
		return source.baseMipLevel() == 0 && t.getWidth(0) == CELL && t.getHeight(0) == CELL && t.getMipLevels() == 1
				&& t.getDepthOrLayers() == 1 && t.getFormat() == GpuFormat.RGBA8_UNORM && !t.isClosed()
				// Made before Polonium could mark it copyable (see SkinTextureUsageMixin): drawn on its own.
				&& (t.usage() & GpuTexture.USAGE_COPY_SRC) != 0;
	}

	/** The texture's cell (copying it in if it's new), or -1 if the atlas can't take it now. */
	int cell(GpuTextureView source, long frame) {
		if (failed) {
			return -1;
		}
		GpuTexture t = source.texture();
		Integer known = cells.get(t);
		if (known != null && !t.isClosed()) {
			lastUsed[known] = frame;
			return known;
		}
		if (known != null) {
			cells.remove(t);
		}
		try {
			int cell = freeCell(frame);
			if (cell < 0) {
				return -1;
			}
			copyIn(t, cell);
			if (owners[cell] != null) {
				cells.remove(owners[cell]);
			}
			owners[cell] = t;
			cells.put(t, cell);
			lastUsed[cell] = frame;
			return cell;
		} catch (RuntimeException e) {
			failed = true;
			org.slf4j.LoggerFactory.getLogger("Polonium").warn("Polonium: skins can't share a texture here; one draw per skin", e);
			return -1;
		}
	}

	/**
	 * The texture's cell if it's in the atlas already (marked used), else -1:
	 * only reads (and marks), so several threads may ask at once while
	 * nothing is added.
	 */
	int knownCell(GpuTextureView source, long frame) {
		if (failed) {
			return -1;
		}
		GpuTexture t = source.texture();
		Integer known = cells.get(t);
		if (known == null || t.isClosed()) {
			return -1;
		}
		lastUsed[known] = frame;
		return known;
	}

	/** The frame is drawn: textures replaced by a bigger atlas can go. */
	void endFrame() {
		for (GpuTextureView v : retiredViews) {
			v.close();
		}
		for (GpuTexture t : retired) {
			t.close();
		}
		retiredViews.clear();
		retired.clear();
	}

	/** A cell nobody uses: never used yet (growing the atlas if it can), its texture gone, or idle for a while. */
	private int freeCell(long frame) {
		if (nextFree == owners.length && size < maxSize()) {
			grow();
		}
		if (nextFree < owners.length) {
			return nextFree++;
		}
		if (fullAt == frame) {
			return -1;
		}
		for (int tries = 0; tries < SCAN_BUDGET && owners.length > 0; tries++) {
			int cell = scan;
			scan = (scan + 1) % owners.length;
			GpuTexture owner = owners[cell];
			if (owner == null || owner.isClosed() || frame - lastUsed[cell] > IDLE_FRAMES) {
				return cell;
			}
		}
		fullAt = frame;
		return -1;
	}

	private int maxSize = -1;

	/** The biggest atlas this GPU takes (asked once). */
	private int maxSize() {
		if (maxSize < 0) {
			int gpu = org.lwjgl.opengl.GL11C.glGetInteger(org.lwjgl.opengl.GL11C.GL_MAX_TEXTURE_SIZE);
			maxSize = Math.max(FIRST_SIZE, Math.min(MAX_SIZE, Integer.highestOneBit(Math.max(gpu, 1))));
		}
		return maxSize;
	}

	/**
	 * A texture twice as wide and tall (or the first one), with the old one
	 * copied into its top-left corner: skins keep their cells, and the new area
	 * becomes new cells.
	 */
	private void grow() {
		int newSize = size == 0 ? FIRST_SIZE : size * 2;
		GpuDevice device = RenderSystem.getDevice();
		GpuTexture bigger = device.createTexture(() -> "Polonium skin atlas", GpuTexture.USAGE_COPY_DST | GpuTexture.USAGE_COPY_SRC
				| GpuTexture.USAGE_TEXTURE_BINDING | GpuTexture.USAGE_RENDER_ATTACHMENT, GpuFormat.RGBA8_UNORM, newSize, newSize, 1, 1);
		if (texture != null) {
			copy(texture, bigger, 0, 0, 0, 0, size, size);
			retired.add(texture);
			retiredViews.add(view);
		}
		// Cells in the new area: the right part of the old rows, then the new rows below.
		int oldPerRow = size / STRIDE;
		int newPerRow = newSize / STRIDE;
		List<int[]> added = new ArrayList<>();
		for (int row = 0; row < newPerRow; row++) {
			for (int column = 0; column < newPerRow; column++) {
				if (row < oldPerRow && column < oldPerRow) {
					continue;
				}
				added.add(new int[] {column * STRIDE + 1, row * STRIDE + 1});
			}
		}
		int old = owners.length;
		cellX = java.util.Arrays.copyOf(cellX, old + added.size());
		cellY = java.util.Arrays.copyOf(cellY, old + added.size());
		owners = java.util.Arrays.copyOf(owners, old + added.size());
		lastUsed = java.util.Arrays.copyOf(lastUsed, old + added.size());
		for (int i = 0; i < added.size(); i++) {
			cellX[old + i] = added.get(i)[0];
			cellY[old + i] = added.get(i)[1];
		}
		texture = bigger;
		view = device.createTextureView(bigger);
		size = newSize;
	}

	/**
	 * The source into its cell, with a one-pixel border repeating its outer
	 * pixels. Copied with OpenGL's glCopyImageSubData, an exact pixel copy:
	 * the game's own texture copy only works into a texture's corner (it
	 * passes the size where the far corner belongs).
	 */
	private void copyIn(GpuTexture source, int cell) {
		int x = cellX[cell];
		int y = cellY[cell];
		int last = CELL - 1;
		copy(source, texture, 0, 0, x, y, CELL, CELL);
		// Borders: edges, then corners.
		copy(source, texture, 0, 0, x, y - 1, CELL, 1);
		copy(source, texture, 0, last, x, y + CELL, CELL, 1);
		copy(source, texture, 0, 0, x - 1, y, 1, CELL);
		copy(source, texture, last, 0, x + CELL, y, 1, CELL);
		copy(source, texture, 0, 0, x - 1, y - 1, 1, 1);
		copy(source, texture, last, 0, x + CELL, y - 1, 1, 1);
		copy(source, texture, 0, last, x - 1, y + CELL, 1, 1);
		copy(source, texture, last, last, x + CELL, y + CELL, 1, 1);
	}

	private static void copy(GpuTexture source, GpuTexture target, int sx, int sy, int dx, int dy, int width, int height) {
		org.lwjgl.opengl.GLCapabilities caps = org.lwjgl.opengl.GL.getCapabilities();
		if (!(source instanceof com.mojang.blaze3d.opengl.GlTexture from) || !(target instanceof com.mojang.blaze3d.opengl.GlTexture to)
				|| !(caps.OpenGL43 || caps.GL_ARB_copy_image)) {
			throw new IllegalStateException("needs OpenGL textures and image copies (source " + source.getClass().getName() + ", GL 4.3 "
					+ caps.OpenGL43 + ", ARB_copy_image " + caps.GL_ARB_copy_image + ")");
		}
		// Core in 4.3; the same entry point as the ARB_copy_image extension.
		org.lwjgl.opengl.ARBCopyImage.glCopyImageSubData(from.glId(), org.lwjgl.opengl.GL11C.GL_TEXTURE_2D, 0, sx, sy, 0,
				to.glId(), org.lwjgl.opengl.GL11C.GL_TEXTURE_2D, 0, dx, dy, 0, width, height, 1);
	}
}
//#endif
