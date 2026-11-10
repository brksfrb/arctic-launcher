package com.arcticlauncher.mod;

//#if MC >= 1.21.6
import java.util.IdentityHashMap;
import org.joml.Matrix4fc;

/** CPU copies of the exact uploaded projections; never read uniform buffers back from the GPU. */
public final class TextProjection {
	private static final IdentityHashMap<Object, Matrix4fc> MATRICES = new IdentityHashMap<>();
	private static Matrix4fc active;
	private static Matrix4fc saved;

	private TextProjection() {}

	public static void remember(Object slice, Matrix4fc matrix) {
		MATRICES.put(slice, matrix);
	}

	public static void forget(Object slice) {
		MATRICES.remove(slice);
	}

	public static void select(Object slice) {
		active = MATRICES.get(slice);
	}

	public static void backup() {
		saved = active;
	}

	public static void restore() {
		active = saved;
	}

	public static Matrix4fc current() {
		return active;
	}
}
//#endif
