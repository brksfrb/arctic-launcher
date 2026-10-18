import java.nio.file.Path;
import java.nio.file.Paths;
import net.fabricmc.tinyremapper.NonClassCopyMode;
import net.fabricmc.tinyremapper.OutputConsumerPath;
import net.fabricmc.tinyremapper.TinyRemapper;
import net.fabricmc.tinyremapper.TinyUtils;
import net.fabricmc.tinyremapper.extension.mixin.MixinExtension;

/** Remap a jar between two namespaces of a Tiny file, mixin annotations included: Remap in out mappings from to classpath... */
public final class Remap {
	public static void main(String[] args) throws Exception {
		Path in = Paths.get(args[0]), out = Paths.get(args[1]), mappings = Paths.get(args[2]);
		Path[] classpath = new Path[args.length - 5];
		for (int i = 5; i < args.length; i++) {
			classpath[i - 5] = Paths.get(args[i]);
		}
		TinyRemapper remapper = TinyRemapper.newRemapper()
				.withMappings(TinyUtils.createTinyMappingProvider(mappings, args[3], args[4]))
				.extension(new MixinExtension())
				.ignoreConflicts(true)
				.build();
		try (OutputConsumerPath output = new OutputConsumerPath.Builder(out).build()) {
			output.addNonClassFiles(in, NonClassCopyMode.FIX_META_INF, remapper);
			remapper.readInputs(in);
			remapper.readClassPath(classpath);
			remapper.apply(output);
		} finally {
			remapper.finish();
		}
	}
}
