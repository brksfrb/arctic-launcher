//#if MC >= 26.2
package com.arcticlauncher.polonium.gpu;

import java.nio.ByteBuffer;
import java.util.Arrays;

/** A growable list of vec4 texels (as floats), reused frame to frame. */
final class InstanceData {
	private float[] floats = new float[4096];
	private int size;
	/** Where it starts in the frame's instance buffer, in texels: set once the frame's batches are laid out, before they're written. */
	int base;

	void put(float x, float y, float z, float w) {
		if (size + 4 > floats.length) {
			floats = Arrays.copyOf(floats, floats.length * 2);
		}
		floats[size] = x;
		floats[size + 1] = y;
		floats[size + 2] = z;
		floats[size + 3] = w;
		size += 4;
	}

	void zeros(int texels) {
		int count = texels * 4;
		if (size + count > floats.length) {
			floats = Arrays.copyOf(floats, Math.max(floats.length * 2, size + count));
		}
		Arrays.fill(floats, size, size + count, 0f);
		size += count;
	}

	/** Room for {@code texels} texels, written later (see {@link #array}); returns where they start, in texels. */
	int reserve(int texels) {
		int count = texels * 4;
		if (size + count > floats.length) {
			floats = Arrays.copyOf(floats, Math.max(floats.length * 2, size + count));
		}
		int start = size / 4;
		size += count;
		return start;
	}

	/** The backing array (stable once nothing more is added this frame). */
	float[] array() {
		return floats;
	}

	int texels() {
		return size / 4;
	}

	void writeTo(ByteBuffer target) {
		target.asFloatBuffer().put(floats, 0, size);
		target.position(target.position() + size * 4);
	}

	void clear() {
		size = 0;
	}
}
//#endif
