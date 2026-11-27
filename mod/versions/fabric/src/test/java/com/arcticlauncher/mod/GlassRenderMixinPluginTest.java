package com.arcticlauncher.mod;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Collections;
import org.junit.jupiter.api.Test;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.MethodNode;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import org.spongepowered.asm.util.Annotations;

class GlassRenderMixinPluginTest {
	private static final String RENDER = "method_3374";
	private static final String WORLD = "Lnet/minecraft/class_1920;";
	private static final String STATE = "Lnet/minecraft/class_2680;";
	private static final String POS = "Lnet/minecraft/class_2338;";

	@Test
	void acceptsBothReturnTypesWithTheirMatchingCallbacks() {
		assertTrue(GlassRenderMixinPlugin.matches(hooks(CallbackInfo.class, CallbackInfo.class),
				renderer("(" + WORLD + STATE + POS + ")V")));
		assertTrue(GlassRenderMixinPlugin.matches(hooks(CallbackInfoReturnable.class, CallbackInfoReturnable.class),
				renderer("(" + WORLD + STATE + POS + ")Z")));
	}

	@Test
	void rejectsEitherMismatchedHandlerBeforeInstallingThePair() {
		ClassNode renderer = renderer("(" + WORLD + STATE + POS + ")V");
		assertFalse(GlassRenderMixinPlugin.matches(hooks(CallbackInfoReturnable.class, CallbackInfoReturnable.class), renderer));
		assertFalse(GlassRenderMixinPlugin.matches(hooks(CallbackInfo.class, CallbackInfoReturnable.class), renderer));
	}

	@Test
	void rejectsMissingOrAmbiguousCapturedArguments() {
		ClassNode hooks = hooks(CallbackInfo.class, CallbackInfo.class);
		assertFalse(GlassRenderMixinPlugin.matches(hooks, renderer("(" + STATE + POS + ")V")));
		assertFalse(GlassRenderMixinPlugin.matches(hooks, renderer("(" + WORLD + STATE + POS + POS + ")V")));
	}

	private static ClassNode hooks(Class<?> beginCallback, Class<?> endCallback) {
		ClassNode hooks = new ClassNode();
		MethodNode begin = new MethodNode(Opcodes.ACC_PRIVATE, "arctic$begin",
				"(" + Type.getDescriptor(beginCallback) + WORLD + STATE + POS + ")V", null, null);
		MethodNode end = new MethodNode(Opcodes.ACC_PRIVATE, "arctic$end",
				"(" + Type.getDescriptor(endCallback) + ")V", null, null);
		for (MethodNode hook : new MethodNode[] {begin, end}) {
			Annotations.setVisible(hook, Inject.class, "method", Collections.singletonList(RENDER));
			hooks.methods.add(hook);
		}
		return hooks;
	}

	private static ClassNode renderer(String descriptor) {
		ClassNode renderer = new ClassNode();
		renderer.methods.add(new MethodNode(Opcodes.ACC_PUBLIC, RENDER, descriptor, null, null));
		return renderer;
	}
}
