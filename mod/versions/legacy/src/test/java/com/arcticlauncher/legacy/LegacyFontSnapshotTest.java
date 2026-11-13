package com.arcticlauncher.legacy;

import com.arcticlauncher.client.feature.TextPixelScale;
import java.io.InputStream;
import java.lang.reflect.Field;
import java.nio.FloatBuffer;
import java.nio.IntBuffer;
import java.util.Arrays;
import java.util.HashSet;
import java.util.Set;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.FieldVisitor;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

import static org.junit.jupiter.api.Assertions.*;

/** Runs the compiled production snapshot methods headlessly; only GL and atlas creation are substituted. */
public class LegacyFontSnapshotTest {
	private Class<?> font;

	@BeforeEach void loadSnapshotMethods() throws Exception {
		String name = "com/arcticlauncher/legacy/LegacySmoothFont";
		Set<String> fields = new HashSet<>(Arrays.asList("MODEL", "PROJECTION", "VIEWPORT", "CLIP", "drawScale", "scale",
				"patcherCheckedAt", "patcherLooked", "patcherOptimized"));
		ClassWriter writer = new ClassWriter(0);
		try (InputStream in = getClass().getResourceAsStream("/" + name + ".class")) {
			assertNotNull(in);
			new ClassReader(in).accept(new ClassVisitor(Opcodes.ASM9, writer) {
				@Override public FieldVisitor visitField(int access, String n, String desc, String sig, Object value) {
					// No game/font fields are needed to execute these two production methods.
					return fields.contains(n) ? super.visitField(access & ~Opcodes.ACC_FINAL, n, desc, sig, value) : null;
				}

				@Override public MethodVisitor visitMethod(int access, String n, String desc, String sig, String[] exceptions) {
					if (!n.equals("beginDraw") && !n.equals("preparing") && !n.equals("yieldPatcher")) return null;
					return new MethodVisitor(Opcodes.ASM9, super.visitMethod(access, n, desc, sig, exceptions)) {
						@Override public void visitMethodInsn(int op, String owner, String method, String descriptor, boolean itf) {
							if (owner.equals("org/lwjgl/opengl/GL11") || owner.equals(name) && method.equals("ready")
									|| owner.equals("java/lang/System") && method.equals("nanoTime")) {
								owner = FakeGl.class.getName().replace('.', '/');
							}
							super.visitMethodInsn(op, owner, method, descriptor, itf);
						}
					};
				}
			}, 0);
		}
		font = new ClassLoader(getClass().getClassLoader()) {
			Class<?> define(byte[] bytes) { return defineClass(null, bytes, 0, bytes.length); }
		}.define(writer.toByteArray());
		set("MODEL", FloatBuffer.allocate(16));
		set("PROJECTION", FloatBuffer.allocate(16));
		set("VIEWPORT", IntBuffer.allocate(16));
		set("CLIP", new float[16]);
		FakeGl.reads = 0;
		FakeGl.clocks = 0;
		FakeGl.nanos = 3_000_000_000L;
		FakeGl.width = 1280;
		FakeGl.atlasReady = false;
		FakeGl.prepareAtlas = () -> {
			try { set("scale", 2); } catch (Exception e) { throw new AssertionError(e); }
		};
		FakeGl.model = new float[] {0.025f, 0, 0.001f, 0, 0, 0.025f, 0, 0, 0, 0, 1, 0, 0, 0, -4, 1};
		FakeGl.projection = new float[] {1, 0, 0, 0, 0, 1, 0, 0, 0, 0, -1, -1, 0, 0, -0.1f, 0};
	}

	@Test void shadowAndForegroundShareThreeReadsButKeepTheirOwnPerspectiveAnchors() throws Exception {
		font.getMethod("beginDraw").invoke(null);
		assertTrue(FakeGl.atlasReady);
		float shadowScale = 0;
		for (float x : new float[] {-100.5f, -100f}) {
			font.getMethod("preparing", float.class, float.class).invoke(null, x, 20f);
			assertEquals(reference(x, 20), scale(), 0f);
			if (shadowScale == 0) shadowScale = scale();
		}
		assertNotEquals(shadowScale, scale());
		assertEquals(3, FakeGl.reads, "two shadow layers previously required six driver reads");
	}

	@Test void separateDrawsObserveDirectTransformsAndViewportChanges() throws Exception {
		font.getMethod("beginDraw").invoke(null);
		font.getMethod("preparing", float.class, float.class).invoke(null, 10f, 20f);
		float first = scale();
		FakeGl.model[14] = -12;
		FakeGl.width = 1920;
		font.getMethod("beginDraw").invoke(null);
		font.getMethod("preparing", float.class, float.class).invoke(null, 10f, 20f);
		assertEquals(reference(10, 20), scale(), 0f);
		assertNotEquals(first, scale());
		assertEquals(6, FakeGl.reads);
	}

	@Test void absentPatcherDoesNotReadClockForEveryGlyph() throws Exception {
		java.lang.reflect.Method poll = font.getDeclaredMethod("yieldPatcher");
		poll.setAccessible(true);
		poll.invoke(null); // Real Class.forName: Patcher is absent from this test runtime.
		int startupClocks = FakeGl.clocks;
		for (int glyph = 0; glyph < 10000; glyph++) poll.invoke(null);
		assertEquals(startupClocks, FakeGl.clocks);
	}

	@Test void installedPatcherStillYieldsAndIsPolledEveryTwoSeconds() throws Exception {
		set("patcherLooked", true);
		set("patcherOptimized", FakePatcher.class.getField("optimizedFontRenderer"));
		java.lang.reflect.Method poll = font.getDeclaredMethod("yieldPatcher");
		poll.setAccessible(true);
		FakePatcher.optimizedFontRenderer = true;
		poll.invoke(null);
		assertFalse(FakePatcher.optimizedFontRenderer);
		FakePatcher.optimizedFontRenderer = true;
		FakeGl.nanos += 1_000_000_000L;
		poll.invoke(null);
		assertTrue(FakePatcher.optimizedFontRenderer);
		FakeGl.nanos += 1_000_000_000L;
		poll.invoke(null);
		assertFalse(FakePatcher.optimizedFontRenderer);
	}

	private float reference(float x, float y) {
		float[] clip = new float[16];
		TextPixelScale.multiply(FloatBuffer.wrap(FakeGl.projection), FloatBuffer.wrap(FakeGl.model), clip);
		return TextPixelScale.projected(clip, x, y, FakeGl.width, 720) / 2;
	}

	private void set(String name, Object value) throws Exception {
		Field field = font.getDeclaredField(name);
		field.setAccessible(true);
		field.set(null, value);
	}

	private float scale() throws Exception {
		Field field = font.getDeclaredField("drawScale");
		field.setAccessible(true);
		return field.getFloat(null);
	}

	public static final class FakeGl {
		static float[] model, projection;
		static int reads;
		static int clocks, width;
		static long nanos;
		static boolean atlasReady;
		static Runnable prepareAtlas;
		public static long nanoTime() { clocks++; return nanos; }
		public static boolean ready() {
			prepareAtlas.run();
			atlasReady = true;
			return true;
		}
		public static void glGetFloat(int what, FloatBuffer out) {
			assertTrue(atlasReady, "GUI divisor must be established before reading transforms");
			reads++;
			float[] values = what == 2982 ? model : projection;
			for (int i = 0; i < 16; i++) out.put(i, values[i]);
		}
		public static void glGetInteger(int what, IntBuffer out) {
			assertEquals(2978, what);
			reads++;
			out.put(2, width).put(3, 720);
		}
	}

	public static final class FakePatcher {
		public static boolean optimizedFontRenderer;
	}
}
