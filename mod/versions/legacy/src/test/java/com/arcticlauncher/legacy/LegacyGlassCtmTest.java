package com.arcticlauncher.legacy;

import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.util.IdentityHashMap;
import java.util.Map;
import net.minecraft.client.render.model.BakedQuad;
import net.minecraft.client.texture.Sprite;
import net.minecraft.util.math.Direction;
import org.junit.jupiter.api.Test;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;

import static org.junit.jupiter.api.Assertions.*;

class LegacyGlassCtmTest {
	@Test
	void skipsUninitializedSubclassButPreparesVanillaGlass() throws Exception {
		Sprite sprite = new Sprite("minecraft:blocks/glass") {};
		sprite.setWidth(16);
		sprite.setHeight(16);
		sprite.reInitialize(16, 16, 0, 0, false);
		Map<Sprite, Sprite[]> atlas = new IdentityHashMap<>();
		Sprite[] tiles = new Sprite[com.arcticlauncher.client.ctm.GlassTiles.COUNT];
		java.util.Arrays.fill(tiles, sprite);
		atlas.put(sprite, tiles);
		Field sprites = LegacyGlassCtm.class.getDeclaredField("sprites");
		sprites.setAccessible(true);
		Object previous = sprites.get(null);
		sprites.set(null, atlas);
		try {
			int[] data = new int[28];
			for (int i = 0; i < 4; i++) {
				data[i * 7 + 4] = Float.floatToRawIntBits(0.5f);
				data[i * 7 + 5] = Float.floatToRawIntBits(0.5f);
			}
			Constructor<?> base = BakedQuad.class.getConstructors()[0];
			// Generate against either legacy constructor (1.8.9 has no Sprite argument).
			Class<?> unpackedType = new FixtureLoader().define(unpackedQuad(base));
			BakedQuad unpacked = (BakedQuad) unpackedType
					.getConstructor(int[].class, Direction.class, Sprite.class)
					.newInstance(data, Direction.NORTH, sprite);
			assertEquals(0, unpackedType.getField("vertexReads").getInt(unpacked),
					"preparation must not dispatch to the subclass getter during construction");
			assertSame(data, unpacked.getVertexData(), "subclass storage is assigned after preparation");
			assertNull(LegacyGlassCtm.prepare(unpacked));
			assertEquals(1, unpackedType.getField("vertexReads").getInt(unpacked));
			BakedQuad vanilla = (BakedQuad) base.newInstance(base.getParameterCount() == 3
					? new Object[]{data, -1, Direction.NORTH}
					: new Object[]{data, -1, Direction.NORTH, sprite});
			LegacyGlassCtm.Variants variants = LegacyGlassCtm.prepare(vanilla);
			assertNotNull(variants, "vanilla glass still gets connected variants");
			assertEquals(tiles.length, variants.quads.length);
		} finally {
			sprites.set(null, previous);
		}
	}

	/** Models the constructor hook before a Forge-style subclass assigns its vertex storage. */
	private static byte[] unpackedQuad(Constructor<?> base) {
		String name = "com/arcticlauncher/legacy/UninitializedQuad";
		String parent = Type.getInternalName(BakedQuad.class);
		String ctm = Type.getInternalName(LegacyGlassCtm.class);
		ClassWriter writer = new ClassWriter(ClassWriter.COMPUTE_MAXS);
		writer.visit(Opcodes.V1_8, Opcodes.ACC_PUBLIC, name, null, parent, null);
		writer.visitField(Opcodes.ACC_PRIVATE, "unpacked", "[I", null, null).visitEnd();
		writer.visitField(Opcodes.ACC_PUBLIC, "vertexReads", "I", null, null).visitEnd();
		MethodVisitor init = writer.visitMethod(Opcodes.ACC_PUBLIC, "<init>",
				Type.getMethodDescriptor(Type.VOID_TYPE, Type.getType(int[].class),
						Type.getType(Direction.class), Type.getType(Sprite.class)), null, null);
		init.visitCode();
		init.visitVarInsn(Opcodes.ALOAD, 0);
		init.visitVarInsn(Opcodes.ALOAD, 1);
		init.visitInsn(Opcodes.ICONST_M1);
		init.visitVarInsn(Opcodes.ALOAD, 2);
		if (base.getParameterCount() == 4) init.visitVarInsn(Opcodes.ALOAD, 3);
		init.visitMethodInsn(Opcodes.INVOKESPECIAL, parent, "<init>", Type.getConstructorDescriptor(base), false);
		// JUnit does not apply mixins: reproduce the call made by the base constructor.
		// Call prepare directly so its exception cannot be hidden by the mixin's fallback.
		init.visitVarInsn(Opcodes.ALOAD, 0);
		init.visitMethodInsn(Opcodes.INVOKESTATIC, ctm, "prepare",
				"(L" + parent + ";)L" + ctm + "$Variants;", false);
		init.visitInsn(Opcodes.POP);
		init.visitVarInsn(Opcodes.ALOAD, 0);
		init.visitVarInsn(Opcodes.ALOAD, 1);
		init.visitFieldInsn(Opcodes.PUTFIELD, name, "unpacked", "[I");
		init.visitInsn(Opcodes.RETURN);
		init.visitMaxs(0, 0);
		init.visitEnd();
		MethodVisitor getter = writer.visitMethod(Opcodes.ACC_PUBLIC, "getVertexData", "()[I", null, null);
		getter.visitCode();
		getter.visitVarInsn(Opcodes.ALOAD, 0);
		getter.visitInsn(Opcodes.DUP);
		getter.visitFieldInsn(Opcodes.GETFIELD, name, "vertexReads", "I");
		getter.visitInsn(Opcodes.ICONST_1);
		getter.visitInsn(Opcodes.IADD);
		getter.visitFieldInsn(Opcodes.PUTFIELD, name, "vertexReads", "I");
		getter.visitVarInsn(Opcodes.ALOAD, 0);
		getter.visitFieldInsn(Opcodes.GETFIELD, name, "unpacked", "[I");
		getter.visitMethodInsn(Opcodes.INVOKESTATIC, "java/util/Objects", "requireNonNull",
				"(Ljava/lang/Object;)Ljava/lang/Object;", false);
		getter.visitTypeInsn(Opcodes.CHECKCAST, "[I");
		getter.visitInsn(Opcodes.ARETURN);
		getter.visitMaxs(0, 0);
		getter.visitEnd();
		writer.visitEnd();
		return writer.toByteArray();
	}

	private static final class FixtureLoader extends ClassLoader {
		FixtureLoader() { super(LegacyGlassCtmTest.class.getClassLoader()); }
		Class<?> define(byte[] bytes) { return defineClass(null, bytes, 0, bytes.length); }
	}
}
