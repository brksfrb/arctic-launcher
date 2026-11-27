package com.arcticlauncher.legacy;

import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.junit.jupiter.api.Test;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

import static org.junit.jupiter.api.Assertions.*;

class LegacyMixinOwnersTest {
	@Test
	void mixinInvocationAndFieldOwnersAreLoadableClassNames() throws Exception {
		Path classes = Paths.get(LegacyGlassCtm.class.getProtectionDomain().getCodeSource().getLocation().toURI());
		Path mixins = classes.resolve("com/arcticlauncher/legacy/mixin");
		List<Path> files;
		try (Stream<Path> paths = Files.walk(mixins)) {
			files = paths.filter(p -> p.toString().endsWith(".class")).collect(Collectors.toList());
		}
		assertFalse(files.isEmpty(), "must inspect compiled mixins");
		for (Path file : files) {
			new ClassReader(Files.readAllBytes(file)).accept(new ClassVisitor(Opcodes.ASM9) {
				@Override
				public MethodVisitor visitMethod(int access, String name, String desc, String signature, String[] exceptions) {
					return new MethodVisitor(Opcodes.ASM9) {
						private void checkOwner(String owner) {
							assertFalse(owner.startsWith("["), file + ": " + name + desc + " uses array owner " + owner);
						}
						@Override
						public void visitMethodInsn(int opcode, String owner, String method, String descriptor, boolean isInterface) {
							checkOwner(owner);
						}
						@Override
						public void visitFieldInsn(int opcode, String owner, String field, String descriptor) {
							checkOwner(owner);
						}
					};
				}
			}, ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
		}
	}
}
