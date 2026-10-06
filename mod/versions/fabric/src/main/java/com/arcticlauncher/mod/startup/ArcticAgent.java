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
		code.add(new InsnNode(Opcodes.DUP));
		code.add(new VarInsnNode(Opcodes.ASTORE, 3));
		code.add(new org.objectweb.asm.tree.JumpInsnNode(Opcodes.IFNULL, miss));
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
