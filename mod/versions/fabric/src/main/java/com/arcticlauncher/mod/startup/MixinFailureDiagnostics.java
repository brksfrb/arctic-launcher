//#if MC >= 1.18
package com.arcticlauncher.mod.startup;

import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeSet;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

import com.arcticlauncher.client.Lang;
/** Agent/system-loader only. No class bytes, mixin lists, or per-class history retained on success. */
public final class MixinFailureDiagnostics {
	private static final String PREFIX = "[Arctic mixin] "; // i18n-ignore: log
	private static final PrintStream OUT = new PrintStream(new FileOutputStream(FileDescriptor.err), true);
	private static final Map<String, String> FAILURES = new LinkedHashMap<>();
	private static Thread mainThread;
	private static boolean reporting;

	private MixinFailureDiagnostics() {}

	static void init() {
		mainThread = Thread.currentThread();
		OUT.println(PREFIX + "agent installed; cache=" + MixinCache.stats() // i18n-ignore
				+ "; preloader requested=" + (System.getProperty("arctic.preload") != null) // i18n-ignore
				+ "; replay requested=" + Boolean.getBoolean("arctic.replay")); // i18n-ignore
	}

	/** Called by the guarded transformClassBytes invocation; the original Throwable is rethrown by Knot. */
	public static synchronized void failure(String name, byte[] input, Object transformer, Throwable error) {
		// Loading diagnostics' own dependencies can reenter Knot. Keep the first/deepest failures
		// and the latest failure (which may be fatal after preloader failures were swallowed).
		if (reporting || FAILURES.containsKey(name)) return;
		reporting = true;
		try {
			Thread thread = Thread.currentThread();
			String role = thread.getName().startsWith("arctic-preload-") ? "preloader"
					: thread == mainThread ? "main" : "other";
			StringBuilder out = new StringBuilder(PREFIX + "failure begin\n"); // i18n-ignore
			line(out, "class=" + name + "; thread=" + thread.getName() + "; role=" + role);
			// This boundary is reached only after the cache wrapper missed. Hits return already-mixed
			// bytes (or NULL_MARK) without invoking Mixin at all, even with a cache index present.
			line(out, "cache lookup=miss; input=live pre-mixin bytes; cache=" + MixinCache.stats()); // i18n-ignore
			line(out, "agent lock=" + System.getProperty("arctic.agent.lock", "0") // i18n-ignore
					+ "; preloader started=" + System.getProperty("arctic.diagnostics.preloader.started", "false")
					+ "; setup replay succeeded=" + MixinCache.active() // i18n-ignore
					+ "; replay plugin calls attempted=" + MixinReplay.callsAttempted // i18n-ignore
					+ "; completed=" + MixinReplay.callsCompleted
					+ "; shouldApply decisions attempted=" + MixinReplay.decisionsAttempted // i18n-ignore
					+ "; completed=" + MixinReplay.decisionsCompleted + "; replay reason=" + MixinReplay.why); // i18n-ignore
			line(out, "error=" + error);
			try {
				line(out, "pre-mixin methods (name+descriptor):"); // i18n-ignore
				out.append(methods(input));
			} catch (Throwable unavailable) {
				line(out, "methods unavailable: " + unavailable.getClass().getSimpleName()); // i18n-ignore
			}
			try {
				line(out, "targeting mixins (config:class; includes failing mixin):"); // i18n-ignore
				TreeSet<String> mixins = targetingMixins(transformer, name);
				line(out, "selected target mixin count=" + mixins.size()); // i18n-ignore
				for (String mixin : mixins) line(out, mixin);
			} catch (Throwable unavailable) {
				line(out, "targeting mixins unavailable: " + unavailable.getClass().getSimpleName()); // i18n-ignore
			}
			line(out, "failure end"); // i18n-ignore
			// Avoid full mod names/paths: just resolved ids and versions, including nested jars.
			try {
				Class<?> api = Class.forName("net.fabricmc.loader.api.FabricLoader", false, transformer.getClass().getClassLoader());
				Object loader = api.getMethod("getInstance").invoke(null);
				TreeSet<String> mods = new TreeSet<>();
				for (Object mod : (Iterable<?>) api.getMethod("getAllMods").invoke(loader)) {
					Object metadata = invoke(mod, "getMetadata");
					mods.add(invoke(metadata, "getId") + " " + invoke(invoke(metadata, "getVersion"), "getFriendlyString"));
				}
				for (String mod : mods) out.append("[Arctic mixin mod] ").append(mod).append('\n'); // i18n-ignore
			} catch (Throwable unavailable) {
				line(out, "resolved mods unavailable: " + unavailable.getClass().getSimpleName()); // i18n-ignore
			}
			String report = out.toString();
			if (FAILURES.size() == 4) {
				String latest = null;
				for (String previous : FAILURES.keySet()) latest = previous;
				FAILURES.remove(latest);
			}
			FAILURES.put(name, report);
			OUT.print(report);
			OUT.flush();
			// Same instance-specific destination convention as CrashDiagnostics' hang sidecar.
			Path target = Path.of(System.getProperty("arctic.diagnostics.hangFile", "arctic-hang.txt"))
					.toAbsolutePath().resolveSibling("arctic-mixin-failure.txt");
			Path temporary = target.resolveSibling(target.getFileName() + ".tmp");
			String reports = String.join("", FAILURES.values());
			if (reports.length() <= 512 * 1024) {
				Files.writeString(temporary, reports);
				Files.move(temporary, target, StandardCopyOption.REPLACE_EXISTING);
			}
		} catch (Throwable ignored) {
			// All diagnostics, including reflection, linkage and disk I/O, are best effort.
		} finally {
			reporting = false;
		}
	}

	static String methods(byte[] input) {
		if (input == null) return PREFIX + "input=null (generated class; no incoming methods)\n";
		StringBuilder out = new StringBuilder();
		new ClassReader(input).accept(new ClassVisitor(Opcodes.ASM9) {
			@Override
			public MethodVisitor visitMethod(int access, String name, String desc, String signature, String[] exceptions) {
				line(out, name + desc);
				return null;
			}
		}, ClassReader.SKIP_CODE | ClassReader.SKIP_DEBUG | ClassReader.SKIP_FRAMES);
		return out.toString();
	}

	/** Read Mixin's selected configs, not Mixins.getConfigs() (which drops prepared configs).
	 * This never calls a plugin, reads another target, or reruns Mixin selection. */
	static TreeSet<String> targetingMixins(Object transformer, String name) throws ReflectiveOperationException {
		Object processor = field(transformer, "processor");
		TreeSet<String> result = new TreeSet<>();
		for (Object config : (Iterable<?>) field(processor, "configs")) {
			Map<?, ?> mapping = (Map<?, ?>) field(config, "mixinMapping");
			Object mixins = mapping.get(name.replace('/', '.'));
			if (!(mixins instanceof Iterable<?> targets)) continue;
			for (Object mixin : targets) {
				result.add(invoke(config, "getName") + ":" + invoke(mixin, "getClassName"));
			}
		}
		return result;
	}

	private static Object field(Object object, String name) throws ReflectiveOperationException {
		Field field = object.getClass().getDeclaredField(name);
		field.setAccessible(true);
		return field.get(object);
	}

	private static Object invoke(Object object, String name) throws ReflectiveOperationException {
		Method method = object.getClass().getMethod(name);
		method.setAccessible(true);
		return method.invoke(object);
	}

	private static void line(StringBuilder out, String text) {
		out.append(PREFIX).append(text).append('\n');
	}
}
//#endif
