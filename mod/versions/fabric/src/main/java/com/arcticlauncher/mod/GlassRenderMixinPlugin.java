package com.arcticlauncher.mod;

import java.io.IOException;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import org.apache.logging.log4j.LogManager;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;
import org.objectweb.asm.tree.AnnotationNode;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.MethodNode;
import org.spongepowered.asm.mixin.extensibility.IMixinConfigPlugin;
import org.spongepowered.asm.mixin.extensibility.IMixinInfo;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;
import org.spongepowered.asm.service.IClassBytecodeProvider;
import org.spongepowered.asm.service.MixinService;
import org.spongepowered.asm.util.Annotations;

/** Check the two connected-glass hooks together before Mixin applies either one. */
public final class GlassRenderMixinPlugin implements IMixinConfigPlugin {
	private final AtomicBoolean warned = new AtomicBoolean();

	@Override
	public boolean shouldApplyMixin(String targetClassName, String mixinClassName) {
		if (!mixinClassName.equals("com.arcticlauncher.mod.mixin.GlassRenderMixin")) return true;
		try {
			IClassBytecodeProvider provider = MixinService.getService().getBytecodeProvider();
			// Read the compiled selectors and callback types in the runtime namespace.
			// This also checks the remapped jar used by the launcher.
			ClassNode hooks = provider.getClassNode(mixinClassName, false, ClassReader.SKIP_CODE);
			ClassNode target = provider.getClassNode(targetClassName, false, ClassReader.SKIP_CODE);
			if (matches(hooks, target)) return true;
		} catch (ClassNotFoundException | IOException | RuntimeException | LinkageError ignored) {
			// The vanilla renderer can run without these cosmetic hooks.
		}
		if (warned.compareAndSet(false, true)) {
			LogManager.getLogger("Arctic").warn("Skipping connected glass: incompatible block renderer {}", targetClassName);
		}
		return false;
	}

	static boolean matches(ClassNode hooks, ClassNode target) {
		int checked = 0;
		for (MethodNode hook : hooks.methods) {
			AnnotationNode inject = Annotations.getVisible(hook, Inject.class);
			if (inject == null) continue;
			List<String> selectors = Annotations.getValue(inject, "method", false);
			if (selectors.size() != 1) return false;

			MethodNode selected = null;
			for (MethodNode method : target.methods) {
				if (!method.name.equals(selectors.get(0))) continue;
				if (selected != null) return false;
				selected = method;
			}
			if (selected == null || !matchesCallback(hook, selected)) return false;
			checked++;
		}
		return checked == 2;
	}

	private static boolean matchesCallback(MethodNode hook, MethodNode target) {
		if ((target.access & (Opcodes.ACC_STATIC | Opcodes.ACC_ABSTRACT | Opcodes.ACC_NATIVE)) != 0) return false;
		Type result = Type.getReturnType(target.desc);
		Type callback;
		if (result.equals(Type.VOID_TYPE)) callback = Type.getType(CallbackInfo.class);
		else if (result.equals(Type.BOOLEAN_TYPE)) callback = Type.getType(CallbackInfoReturnable.class);
		else return false;

		Type[] handlerArgs = Type.getArgumentTypes(hook.desc);
		if (handlerArgs.length == 0 || !handlerArgs[0].equals(callback)) return false;
		List<Type> targetArgs = Arrays.asList(Type.getArgumentTypes(target.desc));
		// Each remaining hook argument is an argsOnly @Local with no ordinal.
		for (int i = 1; i < handlerArgs.length; i++) {
			if (Collections.frequency(targetArgs, handlerArgs[i]) != 1) return false;
		}
		return Type.getReturnType(hook.desc).equals(Type.VOID_TYPE);
	}

	@Override public void onLoad(String mixinPackage) {}
	@Override public String getRefMapperConfig() { return null; }
	@Override public void acceptTargets(Set<String> myTargets, Set<String> otherTargets) {}
	@Override public List<String> getMixins() { return null; }
	@Override public void preApply(String targetClassName, ClassNode targetClass, String mixinClassName, IMixinInfo mixinInfo) {}
	@Override public void postApply(String targetClassName, ClassNode targetClass, String mixinClassName, IMixinInfo mixinInfo) {}
}
