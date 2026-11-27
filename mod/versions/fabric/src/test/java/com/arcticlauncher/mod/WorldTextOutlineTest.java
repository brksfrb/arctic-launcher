//#if MC >= 26.2
package com.arcticlauncher.mod;

import static org.junit.jupiter.api.Assertions.*;

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.tree.AnnotationNode;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.MethodInsnNode;
import org.objectweb.asm.tree.MethodNode;

/**
 * Glow-ink sign text on 26.2+: the text and its 8x outline must take their letters from the same size bucket.
 * The text gets them through ScaledGlyphMixin (the builder's codepoint accept); the outline looks its letters up
 * in its own lambda and hands them over baked, so it needs WorldTextOutlineMixin. Without it, the outline is drawn
 * from magnified GUI-size letters: a smeared, doubled rim around sharp text (reported on 26.2).
 */
class WorldTextOutlineTest {
	private static final String FONT = "net/minecraft/client/gui/Font";
	private static final String BUILDER = "net/minecraft/client/gui/Font$PreparedTextBuilder";
	private static final String GET_GLYPH = "(ILnet/minecraft/network/chat/Style;)Lnet/minecraft/client/gui/font/glyphs/BakedGlyph;";
	private static final String OUTLINE_LAMBDA = "lambda$prepare8xTextOutline$0";

	@Test
	void outlineLooksUpItsOwnLettersPastTheTextHook() throws IOException {
		MethodNode lambda = method(read(FONT), OUTLINE_LAMBDA);
		assertNotNull(lambda, "the 8x outline lambda moved: WorldTextOutlineMixin no longer applies");
		assertTrue(calls(lambda, FONT, "getGlyph", GET_GLYPH), "outline lambda no longer calls Font.getGlyph");
		assertTrue(calls(lambda, BUILDER, "accept",
				"(ILnet/minecraft/network/chat/Style;Lnet/minecraft/client/gui/font/glyphs/BakedGlyph;)Z"),
				"outline lambda no longer hands baked glyphs to the builder");

		MethodNode textAccept = method(read(BUILDER), "accept", "(ILnet/minecraft/network/chat/Style;I)Z");
		assertNotNull(textAccept);
		assertTrue(calls(textAccept, FONT, "getGlyph", GET_GLYPH), "ScaledGlyphMixin's target moved");
	}

	@Test
	void outlineHookWrapsTheOutlineGlyphLookup() throws IOException {
		ClassNode mixin = read("com/arcticlauncher/mod/mixin/WorldTextOutlineMixin");
		List<String> wrapped = new ArrayList<>();
		for (MethodNode m : mixin.methods) {
			List<AnnotationNode> annotations = new ArrayList<>();
			if (m.visibleAnnotations != null) annotations.addAll(m.visibleAnnotations);
			if (m.invisibleAnnotations != null) annotations.addAll(m.invisibleAnnotations);
			for (AnnotationNode a : annotations) {
				if (a.desc.endsWith("/WrapOperation;")) {
					wrapped.add(String.valueOf(value(a, "method")) + " " + String.valueOf(value((AnnotationNode) first(value(a, "at")), "target")));
				}
			}
		}
		assertEquals(List.of("[" + OUTLINE_LAMBDA + "] L" + FONT + ";getGlyph" + GET_GLYPH), wrapped);
	}

	@Test
	void outlineHookShipsWithTheWorldTextScale() throws IOException {
		String config;
		try (InputStream in = WorldTextOutlineTest.class.getClassLoader().getResourceAsStream("arctic.mixins.json")) {
			assertNotNull(in, "arctic.mixins.json missing from the test classpath");
			config = new String(in.readAllBytes(), StandardCharsets.UTF_8);
		}
		assertTrue(config.contains("\"WorldTextScaleMixin\""));
		assertTrue(config.contains("\"ScaledGlyphMixin\""));
		assertTrue(config.contains("\"WorldTextOutlineMixin\""), "outline hook not in the mixin config");
	}

	private static ClassNode read(String name) throws IOException {
		try (InputStream in = WorldTextOutlineTest.class.getClassLoader().getResourceAsStream(name + ".class")) {
			assertNotNull(in, name + " not on the test classpath");
			ClassNode node = new ClassNode();
			new ClassReader(in).accept(node, ClassReader.SKIP_FRAMES);
			return node;
		}
	}

	private static MethodNode method(ClassNode owner, String name) {
		return owner.methods.stream().filter(m -> m.name.equals(name)).findFirst().orElse(null);
	}

	private static MethodNode method(ClassNode owner, String name, String desc) {
		return owner.methods.stream().filter(m -> m.name.equals(name) && m.desc.equals(desc)).findFirst().orElse(null);
	}

	private static boolean calls(MethodNode m, String owner, String name, String desc) {
		for (var insn : m.instructions) {
			if (insn instanceof MethodInsnNode call && call.owner.equals(owner) && call.name.equals(name) && call.desc.equals(desc)) {
				return true;
			}
		}
		return false;
	}

	private static Object value(AnnotationNode a, String key) {
		for (int i = 0; a.values != null && i < a.values.size(); i += 2) {
			if (a.values.get(i).equals(key)) return a.values.get(i + 1);
		}
		return null;
	}

	private static Object first(Object list) {
		return list instanceof List<?> l && !l.isEmpty() ? l.get(0) : list;
	}
}
//#endif
