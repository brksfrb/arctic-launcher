//#if MC >= 1.18
package com.arcticlauncher.mod.startup;

import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import java.util.ArrayList;
import java.util.List;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.FrameNode;
import org.objectweb.asm.tree.InsnList;
import org.objectweb.asm.tree.InsnNode;
import org.objectweb.asm.tree.LabelNode;
import org.objectweb.asm.tree.LdcInsnNode;
import org.objectweb.asm.tree.MethodInsnNode;
import org.objectweb.asm.tree.MethodNode;
import org.objectweb.asm.tree.TryCatchBlockNode;
import org.objectweb.asm.tree.VarInsnNode;

/**
 * Started by the JVM before the game ({@code -javaagent}, the Arctic Client's own jar): makes
 * Fabric's class loading take turns. Fabric loads each class under a lock of its own and
 * rewrites it under Mixin's lock; with two threads loading at once those two kinds of lock
 * can end up each waiting for the other. One lock around the whole of loading a class rules
 * that out, which is what lets the {@link Preloader} load classes ahead of the game on
 * another thread.
 */
public final class ArcticAgent {
	private static final String DELEGATE = "net/fabricmc/loader/impl/launch/knot/KnotClassDelegate";
	private static final String PLUGIN_HANDLE = "org/spongepowered/asm/mixin/transformer/PluginHandle";
	private static final String MIXIN_CONFIG = "org/spongepowered/asm/mixin/transformer/MixinConfig";
	private static final String REPLAY = "com/arcticlauncher/mod/startup/MixinReplay";
	private static final String LOAD = "loadClass";
	private static final String LOAD_DESC = "(Ljava/lang/String;Z)Ljava/lang/Class;";
	private static final String INNER = "loadClass$arctic";
	private static final String POST = "getPostMixinClassByteArray";
	private static final String POST_DESC = "(Ljava/lang/String;Z)[B";

	private ArcticAgent() {}

	public static void premain(String args, Instrumentation instrumentation) {
		MixinCache.init();
		instrumentation.addTransformer(new ClassFileTransformer() {
			@Override
			public byte[] transform(ClassLoader loader, String name, Class<?> redefined, ProtectionDomain domain, byte[] bytes) {
				if (MIXIN_CONFIG.equals(name)) {
					try {
						return wrapConfig(bytes);
					} catch (Throwable e) {
						return null;
					}
				}
				if (PLUGIN_HANDLE.equals(name)) {
					try {
						return wrapHandle(bytes);
					} catch (Throwable e) {
						return null;
					}
				}
				if (!DELEGATE.equals(name)) {
					// Classes made up while the game runs (MixinExtras' helpers) are defined without Fabric's loading
					// path: the pack needs them too.
					if (name != null && loader != null && MixinCache.recording() && loader.getClass().getName().startsWith("net.fabricmc.loader.impl.launch.knot.Knot")) {
						MixinCache.defined(name.replace('/', '.'), bytes, loader);
					}
					return null;
				}
				try {
					byte[] changed = wrap(bytes);
					System.setProperty("arctic.agent.lock", "1");
					return changed;
				} catch (Throwable e) {
					// Not this Fabric Loader's shape: left as it is, and no class pre-loading then.
					return null;
				}
			}
		});
	}

	/** {@code loadClass(name, resolve)} becomes {@code synchronized (KnotClassDelegate.class) { return loadClass$arctic(name, resolve); }}. */
	static byte[] wrap(byte[] bytes) {
		ClassNode node = new ClassNode();
		new ClassReader(bytes).accept(node, 0);
		lockLoading(node);
		cacheTransformed(node);
		hideServed(node);
		// The class's own frames stay as they are; only the wrappers' are written above.
		ClassWriter writer = new ClassWriter(ClassWriter.COMPUTE_MAXS);
		node.accept(writer);
		return writer.toByteArray();
	}

	/**
	 * {@code getPostMixinClassByteArray(name, fromParent)} (the class as Fabric and Mixin leave it)
	 * asks the {@link MixinCache} first; what it has to produce itself it hands over to be saved.
	 */
	private static void cacheTransformed(ClassNode node) {
		MethodNode original = null;
		for (MethodNode method : node.methods) {
			if (POST.equals(method.name) && POST_DESC.equals(method.desc)) {
				original = method;
			}
		}
		if (original == null || (original.access & Opcodes.ACC_ABSTRACT) != 0) {
			return;
		}
		MethodNode wrapper = new MethodNode(original.access, POST, POST_DESC, original.signature, original.exceptions.toArray(new String[0]));
		original.name = POST + "$arctic";
		original.access = (original.access & ~(Opcodes.ACC_PUBLIC | Opcodes.ACC_PROTECTED)) | Opcodes.ACC_PRIVATE | Opcodes.ACC_SYNTHETIC;
		String cache = "com/arcticlauncher/mod/startup/MixinCache";
		LabelNode miss = new LabelNode();
		InsnList code = new InsnList();
		code.add(new VarInsnNode(Opcodes.ALOAD, 1));
		code.add(new VarInsnNode(Opcodes.ALOAD, 0));
		code.add(new org.objectweb.asm.tree.FieldInsnNode(Opcodes.GETFIELD, node.name, "transformInitialized", "Z"));
		code.add(new MethodInsnNode(Opcodes.INVOKESTATIC, cache, "get", "(Ljava/lang/String;Z)[B", false));
		code.add(new VarInsnNode(Opcodes.ASTORE, 3));
		code.add(new VarInsnNode(Opcodes.ALOAD, 3));
		code.add(new org.objectweb.asm.tree.JumpInsnNode(Opcodes.IFNULL, miss));
		code.add(new VarInsnNode(Opcodes.ALOAD, 3));
		code.add(new org.objectweb.asm.tree.FieldInsnNode(Opcodes.GETSTATIC, cache, "NULL_MARK", "[B"));
		LabelNode hit = new LabelNode();
		code.add(new org.objectweb.asm.tree.JumpInsnNode(Opcodes.IF_ACMPNE, hit));
		code.add(new InsnNode(Opcodes.ACONST_NULL));
		code.add(new InsnNode(Opcodes.ARETURN));
		code.add(hit);
		code.add(new FrameNode(Opcodes.F_FULL, 4, new Object[] {node.name, "java/lang/String", Opcodes.INTEGER, "[B"}, 0, new Object[0]));
		code.add(new VarInsnNode(Opcodes.ALOAD, 3));
		code.add(new InsnNode(Opcodes.ARETURN));
		code.add(miss);
		code.add(new FrameNode(Opcodes.F_FULL, 4, new Object[] {node.name, "java/lang/String", Opcodes.INTEGER, "[B"}, 0, new Object[0]));
		code.add(new VarInsnNode(Opcodes.ALOAD, 0));
		code.add(new VarInsnNode(Opcodes.ALOAD, 1));
		code.add(new VarInsnNode(Opcodes.ILOAD, 2));
		code.add(new MethodInsnNode(Opcodes.INVOKESPECIAL, node.name, original.name, POST_DESC, false));
		code.add(new VarInsnNode(Opcodes.ASTORE, 3));
		code.add(new VarInsnNode(Opcodes.ALOAD, 1));
		code.add(new VarInsnNode(Opcodes.ALOAD, 3));
		code.add(new MethodInsnNode(Opcodes.INVOKESTATIC, cache, "put", "(Ljava/lang/String;[B)V", false));
		code.add(new VarInsnNode(Opcodes.ALOAD, 3));
		code.add(new InsnNode(Opcodes.ARETURN));
		wrapper.instructions = code;
		wrapper.maxLocals = 4;
		wrapper.maxStack = 3;
		node.methods.add(wrapper);
	}

	/**
	 * {@code MixinConfig.onSelect()} creates the mod's plugin and runs its {@code onLoad}. The
	 * {@link MixinReplay} does that ahead of Mixin's own setup; if the setup runs later after all (for a
	 * class the cache lacks) it must not create the plugin a second time.
	 */
	static byte[] wrapConfig(byte[] bytes) {
		ClassNode node = new ClassNode();
		new ClassReader(bytes).accept(node, 0);
		MethodNode select = null;
		boolean hasPlugin = false;
		for (MethodNode method : node.methods) {
			if ("onSelect".equals(method.name) && "()V".equals(method.desc)) {
				select = method;
			}
		}
		for (org.objectweb.asm.tree.FieldNode field : node.fields) {
			if ("plugin".equals(field.name)) {
				hasPlugin = true;
			}
		}
		if (select == null || !hasPlugin) {
			throw new IllegalStateException("not the expected MixinConfig");
		}
		LabelNode go = new LabelNode();
		InsnList guard = new InsnList();
		guard.add(new VarInsnNode(Opcodes.ALOAD, 0));
		guard.add(new org.objectweb.asm.tree.FieldInsnNode(Opcodes.GETFIELD, node.name, "plugin", "Lorg/spongepowered/asm/mixin/transformer/PluginHandle;"));
		guard.add(new org.objectweb.asm.tree.JumpInsnNode(Opcodes.IFNULL, go));
		guard.add(new InsnNode(Opcodes.RETURN));
		guard.add(go);
		guard.add(new FrameNode(Opcodes.F_SAME, 0, null, 0, null));
		select.instructions.insert(guard);
		select.maxStack = Math.max(select.maxStack, 1);
		ClassWriter writer = new ClassWriter(ClassWriter.COMPUTE_MAXS);
		node.accept(writer);
		return writer.toByteArray();
	}

	/**
	 * {@code PluginHandle.shouldApplyMixin(target, mixin)} (Mixin asking a mod's plugin) is reported to the
	 * {@link MixinReplay}, which writes the calls down for the next start.
	 */
	static byte[] wrapHandle(byte[] bytes) {
		ClassNode node = new ClassNode();
		new ClassReader(bytes).accept(node, 0);
		MethodNode apply = null;
		for (MethodNode method : node.methods) {
			if ("shouldApplyMixin".equals(method.name) && "(Ljava/lang/String;Ljava/lang/String;)Z".equals(method.desc)) {
				apply = method;
			}
		}
		if (apply == null) {
			throw new IllegalStateException("not the expected PluginHandle");
		}
		InsnList note = new InsnList();
		note.add(new VarInsnNode(Opcodes.ALOAD, 0));
		note.add(new VarInsnNode(Opcodes.ALOAD, 1));
		note.add(new VarInsnNode(Opcodes.ALOAD, 2));
		note.add(new MethodInsnNode(Opcodes.INVOKESTATIC, REPLAY, "noteApply", "(Ljava/lang/Object;Ljava/lang/String;Ljava/lang/String;)V", false));
		apply.instructions.insert(note);
		apply.maxStack = Math.max(apply.maxStack, 3);
		ClassWriter writer = new ClassWriter(ClassWriter.COMPUTE_MAXS);
		node.accept(writer);
		return writer.toByteArray();
	}

	/**
	 * {@code isClassLoaded(name)} is what Mixin asks to learn whether a target was loaded before its mixins
	 * were looked at (a critical error). Classes read back from the {@link MixinCache} were loaded without
	 * Mixin's help, so if Mixin does its own setup later (for a class the cache didn't have) they must not
	 * count: their mixins are already in them.
	 */
	private static void hideServed(ClassNode node) {
		MethodNode original = null;
		for (MethodNode method : node.methods) {
			if ("isClassLoaded".equals(method.name) && "(Ljava/lang/String;)Z".equals(method.desc)) {
				original = method;
			}
		}
		if (original == null || (original.access & Opcodes.ACC_ABSTRACT) != 0) {
			return;
		}
		MethodNode wrapper = new MethodNode(original.access, "isClassLoaded", "(Ljava/lang/String;)Z", original.signature, null);
		original.name = "isClassLoaded$arctic";
		original.access = (original.access & ~(Opcodes.ACC_PUBLIC | Opcodes.ACC_PROTECTED)) | Opcodes.ACC_PRIVATE | Opcodes.ACC_SYNTHETIC;
		LabelNode loaded = new LabelNode();
		InsnList code = new InsnList();
		code.add(new VarInsnNode(Opcodes.ALOAD, 1));
		code.add(new MethodInsnNode(Opcodes.INVOKESTATIC, "com/arcticlauncher/mod/startup/MixinCache", "served", "(Ljava/lang/String;)Z", false));
		code.add(new org.objectweb.asm.tree.JumpInsnNode(Opcodes.IFEQ, loaded));
		code.add(new InsnNode(Opcodes.ICONST_0));
		code.add(new InsnNode(Opcodes.IRETURN));
		code.add(loaded);
		code.add(new FrameNode(Opcodes.F_SAME, 0, null, 0, null));
		code.add(new VarInsnNode(Opcodes.ALOAD, 0));
		code.add(new VarInsnNode(Opcodes.ALOAD, 1));
		code.add(new MethodInsnNode(Opcodes.INVOKESPECIAL, node.name, original.name, "(Ljava/lang/String;)Z", false));
		code.add(new InsnNode(Opcodes.IRETURN));
		wrapper.instructions = code;
		wrapper.maxLocals = 2;
		wrapper.maxStack = 2;
		node.methods.add(wrapper);
	}

	private static void lockLoading(ClassNode node) {
		MethodNode original = null;
		for (MethodNode method : node.methods) {
			if (LOAD.equals(method.name) && LOAD_DESC.equals(method.desc)) {
				original = method;
			}
		}
		if (original == null || (original.access & Opcodes.ACC_ABSTRACT) != 0) {
			throw new IllegalStateException("no loadClass");
		}
		MethodNode wrapper = new MethodNode(original.access & ~Opcodes.ACC_SYNCHRONIZED, LOAD, LOAD_DESC, original.signature,
				original.exceptions.toArray(new String[0]));
		original.name = INNER;
		original.access = (original.access & ~(Opcodes.ACC_PUBLIC | Opcodes.ACC_PROTECTED | Opcodes.ACC_FINAL)) | Opcodes.ACC_PRIVATE | Opcodes.ACC_SYNTHETIC;
		LabelNode start = new LabelNode();
		LabelNode end = new LabelNode();
		LabelNode handler = new LabelNode();
		InsnList code = new InsnList();
		code.add(new LdcInsnNode(Type.getObjectType(node.name)));
		code.add(new InsnNode(Opcodes.DUP));
		code.add(new VarInsnNode(Opcodes.ASTORE, 3));
		code.add(new InsnNode(Opcodes.MONITORENTER));
		code.add(start);
		code.add(new VarInsnNode(Opcodes.ALOAD, 0));
		code.add(new VarInsnNode(Opcodes.ALOAD, 1));
		code.add(new VarInsnNode(Opcodes.ILOAD, 2));
		code.add(new MethodInsnNode(Opcodes.INVOKESPECIAL, node.name, INNER, LOAD_DESC, false));
		code.add(end);
		code.add(new VarInsnNode(Opcodes.ALOAD, 3));
		code.add(new InsnNode(Opcodes.MONITOREXIT));
		code.add(new InsnNode(Opcodes.ARETURN));
		code.add(handler);
		// The one place the code can be jumped to: this, name, resolve, the lock; the exception on the stack.
		code.add(new FrameNode(Opcodes.F_FULL, 4, new Object[] {node.name, "java/lang/String", Opcodes.INTEGER, "java/lang/Class"}, 1,
				new Object[] {"java/lang/Throwable"}));
		code.add(new VarInsnNode(Opcodes.ASTORE, 4));
		code.add(new VarInsnNode(Opcodes.ALOAD, 3));
		code.add(new InsnNode(Opcodes.MONITOREXIT));
		code.add(new VarInsnNode(Opcodes.ALOAD, 4));
		code.add(new InsnNode(Opcodes.ATHROW));
		wrapper.instructions = code;
		wrapper.tryCatchBlocks = new ArrayList<>(List.of(new TryCatchBlockNode(start, end, handler, null)));
		wrapper.maxLocals = 5;
		wrapper.maxStack = 4;
		node.methods.add(wrapper);
	}
}
//#endif
