package joinagent;

import java.io.File;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

/**
 * Measurement only. Args: outDir;action;delayMs;sampleMs
 * action = mp:host:port | sp:worldFolder | none. Hooks the buffer swap (frame clock + action trigger)
 * and the game's set-screen method (screen timeline), for 1.8.9 (legacy intermediary), 1.21.11
 * (intermediary) and 26.x (Mojang names).
 */
public final class Agent {
	public static void premain(String args, Instrumentation inst) throws Exception {
		String[] a = args.split(";");
		File clock = new File(new File(Agent.class.getProtectionDomain().getCodeSource().getLocation().toURI()).getParentFile(), "joinclock.jar");
		inst.appendToBootstrapClassLoaderSearch(new java.util.jar.JarFile(clock));
		Class<?> j = Class.forName("joinagent.J", true, null);
		j.getMethod("init", String.class, String.class, long.class, int.class)
				.invoke(null, a[0], a.length > 1 ? a[1] : "none", a.length > 2 ? Long.parseLong(a[2]) : 20000L, a.length > 3 ? Integer.parseInt(a[3]) : 5);
		final java.lang.reflect.Method loader = j.getMethod("loader", String.class, ClassLoader.class);
		inst.addTransformer(new ClassFileTransformer() {
			public byte[] transform(ClassLoader l, String name, Class<?> c, ProtectionDomain d, byte[] data) {
				if (name == null) return null;
				if (name.equals("org/lwjgl/opengl/Display")) return hook(data, "update", "()V", "frame", false);
				if (name.equals("org/lwjgl/glfw/GLFW")) return hook(data, "glfwSwapBuffers", "(J)V", "frame", false);
				if (name.equals("org/lwjgl/sdl/SDLVideo")) return hook(data, "SDL_GL_SwapWindow", "(J)Z", "frame", false);
				String v = null, m = null, desc = null;
				if (name.equals("net/minecraft/class_1600")) { v = "189"; m = "method_2928"; desc = "(Lnet/minecraft/class_388;)V"; }
				else if (name.equals("net/minecraft/class_310")) { v = "1211"; m = "method_1507"; desc = "(Lnet/minecraft/class_437;)V"; }
				else if (name.equals("net/minecraft/client/gui/Gui")) { v = "26"; m = "setScreen"; desc = "(Lnet/minecraft/client/gui/screens/Screen;)V"; }
				else if (name.equals("net/minecraft/client/Minecraft")) { try { loader.invoke(null, "26", l); } catch (Throwable x) { x.printStackTrace(); } return null; }
				if (v == null) return null;
				try { loader.invoke(null, v, l); } catch (Throwable x) { x.printStackTrace(); }
				return hook(data, m, desc, "screen", true);
			}
		});
	}

	static byte[] hook(byte[] data, final String method, final String desc, final String target, final boolean arg) {
		try {
			ClassReader r = new ClassReader(data);
			ClassWriter w = new ClassWriter(r, ClassWriter.COMPUTE_MAXS);
			r.accept(new ClassVisitor(Opcodes.ASM9, w) {
				public MethodVisitor visitMethod(int acc, String n, String d, String s, String[] e) {
					MethodVisitor mv = super.visitMethod(acc, n, d, s, e);
					if (!n.equals(method) || !d.equals(desc)) return mv;
					System.out.println("[joinagent] hooked " + n + d);
					return new MethodVisitor(Opcodes.ASM9, mv) {
						public void visitCode() {
							super.visitCode();
							if (arg) {
								super.visitVarInsn(Opcodes.ALOAD, 1);
								super.visitMethodInsn(Opcodes.INVOKESTATIC, "joinagent/J", target, "(Ljava/lang/Object;)V", false);
							} else {
								super.visitMethodInsn(Opcodes.INVOKESTATIC, "joinagent/J", target, "()V", false);
							}
						}
					};
				}
			}, 0);
			return w.toByteArray();
		} catch (Throwable x) {
			x.printStackTrace();
			return null;
		}
	}
}
