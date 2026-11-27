package auditagent;

import java.io.File;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

/** Measurement only. Args: outDir[;sampleMs]. Hooks the buffer swap (frame clock) on LWJGL 2, GLFW and SDL. */
public final class Agent {
	public static void premain(String args, Instrumentation inst) throws Exception {
		String[] a = args.split(";");
		File clock = new File(new File(Agent.class.getProtectionDomain().getCodeSource().getLocation().toURI()).getParentFile(), "auditclock.jar");
		inst.appendToBootstrapClassLoaderSearch(new java.util.jar.JarFile(clock));
		Class<?> c = Class.forName("auditagent.A", true, null);
		c.getMethod("init", String.class, int.class).invoke(null, a[0], a.length > 1 ? Integer.parseInt(a[1]) : 5);
		inst.addTransformer(new ClassFileTransformer() {
			public byte[] transform(ClassLoader l, String name, Class<?> k, ProtectionDomain d, byte[] data) {
				if (name == null) return null;
				if (name.equals("org/lwjgl/opengl/Display")) return hook(data, "update", "()V");
				if (name.equals("org/lwjgl/glfw/GLFW")) return hook(data, "glfwSwapBuffers", "(J)V");
				if (name.equals("org/lwjgl/sdl/SDLVideo")) return hook(data, "SDL_GL_SwapWindow", "(J)Z");
				return null;
			}
		});
	}

	static byte[] hook(byte[] data, final String method, final String desc) {
		try {
			ClassReader r = new ClassReader(data);
			ClassWriter w = new ClassWriter(r, ClassWriter.COMPUTE_MAXS);
			r.accept(new ClassVisitor(Opcodes.ASM9, w) {
				public MethodVisitor visitMethod(int acc, String n, String d, String s, String[] e) {
					MethodVisitor mv = super.visitMethod(acc, n, d, s, e);
					if (!n.equals(method) || !d.equals(desc)) return mv;
					System.out.println("[auditagent] hooked " + n + d);
					return new MethodVisitor(Opcodes.ASM9, mv) {
						public void visitCode() {
							super.visitCode();
							super.visitMethodInsn(Opcodes.INVOKESTATIC, "auditagent/A", "frame", "()V", false);
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
