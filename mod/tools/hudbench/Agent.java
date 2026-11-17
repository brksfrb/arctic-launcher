package polr2;
import java.lang.instrument.*;
import java.security.ProtectionDomain;
import org.objectweb.asm.*;
public final class Agent {
 public static void premain(String arg, Instrumentation inst) throws Exception {
  inst.appendToBootstrapClassLoaderSearch(new java.util.jar.JarFile(arg));
  inst.addTransformer(new ClassFileTransformer() {
   public byte[] transform(ClassLoader loader, String name, Class<?> c, ProtectionDomain domain, byte[] bytes) {
    if (!name.equals("com/arcticlauncher/client/hud/Hud")) return null;
    ClassReader r = new ClassReader(bytes); ClassWriter w = new ClassWriter(r, ClassWriter.COMPUTE_MAXS);
    r.accept(new ClassVisitor(Opcodes.ASM9, w) {
     public MethodVisitor visitMethod(int a, String n, String d, String s, String[] e) {
      MethodVisitor m = super.visitMethod(a,n,d,s,e);
      if (!n.equals("render")) return m;
      return new MethodVisitor(Opcodes.ASM9,m) {
       public void visitCode() { super.visitCode(); super.visitMethodInsn(Opcodes.INVOKESTATIC,"polr2/Meter","begin","()V",false); }
       public void visitInsn(int op) { if (op == Opcodes.RETURN) super.visitMethodInsn(Opcodes.INVOKESTATIC,"polr2/Meter","end","()V",false); super.visitInsn(op); }
      };
     }
    },0); return w.toByteArray();
   }
  });
 }
}
