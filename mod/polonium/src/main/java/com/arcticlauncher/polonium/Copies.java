package com.arcticlauncher.polonium;

import com.arcticlauncher.polonium.mixin.DrawAccess;
import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import java.nio.ByteBuffer;
import java.util.Arrays;
import java.util.List;
import net.minecraft.client.renderer.StagedVertexBuffer;
import org.lwjgl.system.MemoryUtil;

/**
 * Every vertex copy of a frame as plain addresses: from each draw's slices to
 * its place in the mapped buffer. The whole lot is split into equal byte
 * ranges, one per thread, copied with raw memory copies: no per-frame objects
 * beyond these arrays, which are reused.
 */
final class Copies {
	private static long[] sources = new long[1024];
	private static int[] targets = new int[1024];
	/** Bytes before each copy (the last entry is the total). */
	private static long[] before = new long[1025];

	private final long base;
	private final int count;

	private Copies(long base, int count) {
		this.base = base;
		this.count = count;
	}

	/** The copies for these draws into {@code buffer}, as the game would make them (render thread only). */
	static Copies of(ByteBuffer buffer, List<StagedVertexBuffer.Draw> draws) {
		int count = 0;
		long total = 0;
		int capacity = buffer.capacity();
		for (StagedVertexBuffer.Draw draw : draws) {
			if (draw.isEmpty()) {
				continue;
			}
			DrawAccess access = (DrawAccess) draw;
			int at = access.polonium$vertexOffset();
			for (ByteBufferBuilder.Result slice : access.polonium$slices()) {
				ByteBuffer source = slice.byteBuffer();
				int length = source.remaining();
				if (at < 0 || at + length > capacity) {
					throw new IllegalStateException("vertex copy outside the buffer: " + at + "+" + length + " > " + capacity);
				}
				grow(count + 1);
				sources[count] = MemoryUtil.memAddress(source);
				targets[count] = at;
				before[count] = total;
				total += length;
				count++;
				at += length;
			}
		}
		before[count] = total;
		return new Copies(MemoryUtil.memAddress(buffer, 0), count);
	}

	/** Copy part {@code part} of {@code parts} equal byte ranges. */
	void copyPart(int part, int parts) {
		long total = before[count];
		long from = total * part / parts;
		long to = total * (part + 1) / parts;
		if (from >= to) {
			return;
		}
		// The copy holding byte `from`: the last one starting at or before it.
		int i = Arrays.binarySearch(before, 0, count, from);
		if (i < 0) {
			i = -i - 2;
		}
		for (; i < count && before[i] < to; i++) {
			long start = Math.max(from, before[i]);
			long end = Math.min(to, before[i + 1]);
			long skip = start - before[i];
			MemoryUtil.memCopy(sources[i] + skip, base + targets[i] + skip, end - start);
		}
	}

	private static void grow(int needed) {
		if (needed < sources.length) {
			return;
		}
		int size = Math.max(needed + 1, sources.length * 2);
		sources = Arrays.copyOf(sources, size);
		targets = Arrays.copyOf(targets, size);
		before = Arrays.copyOf(before, size + 1);
	}
}
