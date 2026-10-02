//#if MC >= 26.2
package com.arcticlauncher.polonium.gpu;

import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.util.function.Function;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import net.minecraft.resources.Identifier;
import org.jspecify.annotations.Nullable;

/**
 * Polonium's vertex shaders, made from the game's own when they're loaded
 * (so they follow it, and resource packs that change it): the game's shader
 * is kept as it is, except that its vertex inputs become plain variables and
 * its main() is renamed; Polonium's part (from its jar) works those inputs out
 * from its per-entity data and then runs the game's main().
 */
public final class ShaderSources {
	/** Polonium's shader → the game's shader it's made from, and Polonium's part. */
	private record Made(Identifier from, String part) {}

	private static final java.util.Map<Identifier, Made> MADE = java.util.Map.of(
			ModelMesh.SHADER, new Made(Identifier.withDefaultNamespace("core/entity"), "/polonium/entity_instanced.glsl"),
			ItemMesh.SHADER, new Made(Identifier.withDefaultNamespace("core/item"), "/polonium/item_instanced.glsl"),
			GpuText.TEXT_SHADER, new Made(Identifier.withDefaultNamespace("core/text"), "/polonium/text_pooled.glsl"),
			GpuText.BACKGROUND_SHADER, new Made(Identifier.withDefaultNamespace("core/text_background"), "/polonium/text_background_pooled.glsl"));
	/** The game's vertex inputs that Polonium supplies (as minecraft_Position, …). */
	private static final String INPUTS = "Position|Color|UV0|UV1|UV2|Normal";
	private static final Pattern VERSION = Pattern.compile("(?m)^#version[^\\n]*\\n");
	private static final Pattern INPUT_DECLARATION = Pattern.compile("(?m)^[ \\t]*in[ \\t]+\\w+[ \\t]+(?:" + INPUTS + ")[ \\t]*;[^\\n]*$");
	private static final Pattern INPUT_USE = Pattern.compile("\\b(" + INPUTS + ")\\b");
	private static final Pattern MAIN = Pattern.compile("\\bvoid\\s+main\\s*\\(\\s*\\)");
	private static final String HEADER = """
			vec3 minecraft_Position;
			vec4 minecraft_Color;
			vec2 minecraft_UV0;
			ivec2 minecraft_UV1;
			ivec2 minecraft_UV2;
			vec3 minecraft_Normal;
			void minecraft_main();
			""";

	private ShaderSources() {}

	/** Whether {@code id} is one of Polonium's vertex shaders. */
	public static boolean ours(Identifier id) {
		return MADE.containsKey(id);
	}

	/**
	 * The source of one of Polonium's vertex shaders, made from the game's
	 * ({@code gameSource}: a loaded vertex shader's source, imports resolved).
	 */
	public static String vertex(Identifier id, Function<Identifier, @Nullable String> gameSource) {
		Made made = MADE.get(id);
		String game = gameSource.apply(made.from());
		if (game == null) {
			throw new IllegalStateException("the game's " + made.from() + " shader isn't loaded");
		}
		return adapt(game, read(made.part()), made.from());
	}

	/** The game's shader with its inputs supplied by {@code part} (which defines polonium_main). */
	static String adapt(String game, String part, Identifier from) {
		Matcher version = VERSION.matcher(game);
		if (!version.find()) {
			throw new IllegalStateException(from + " has no #version line");
		}
		String body = game.substring(version.end());
		body = INPUT_DECLARATION.matcher(body).replaceAll("");
		Matcher main = MAIN.matcher(body);
		if (!main.find()) {
			throw new IllegalStateException(from + " has no main()");
		}
		int at = main.start();
		if (main.find()) {
			throw new IllegalStateException(from + " has more than one main()");
		}
		body = body.substring(0, at) + "void minecraft_main()" + body.substring(at).replaceFirst(MAIN.pattern(), "");
		body = INPUT_USE.matcher(body).replaceAll("minecraft_$1");
		return game.substring(0, version.end()) + HEADER + body + "\n" + part + "\nvoid main() {\n    polonium_main();\n}\n";
	}

	private static String read(String file) {
		try (InputStream in = ShaderSources.class.getResourceAsStream(file)) {
			if (in == null) {
				throw new IllegalStateException(file + " is missing from the jar");
			}
			return new String(in.readAllBytes(), StandardCharsets.UTF_8);
		} catch (IOException e) {
			throw new UncheckedIOException(e);
		}
	}
}
//#endif
