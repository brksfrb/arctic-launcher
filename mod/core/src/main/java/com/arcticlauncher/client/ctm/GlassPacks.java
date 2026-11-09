package com.arcticlauncher.client.ctm;

import java.awt.image.BufferedImage;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Properties;
import javax.imageio.ImageIO;

/** Loads full-method OptiFine/MCPatcher glass tiles once per resource reload. */
public final class GlassPacks {
	public interface Resources {
		Collection<String> properties() throws IOException;
		InputStream open(String name) throws IOException;
	}
	private static final String[] COLORS = {"white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"};
	private final Map<String, BufferedImage[]> tiles;
	public static final GlassPacks EMPTY = new GlassPacks(Collections.emptyMap());
	private GlassPacks(Map<String, BufferedImage[]> tiles) { this.tiles = tiles; }

	public static GlassPacks load(Resources resources) {
		Map<String, BufferedImage[]> tiles = new HashMap<>();
		try {
			List<String> paths = new ArrayList<>(resources.properties());
			for (String path : paths) {
				try (InputStream stream = resources.open(path)) {
					Properties p = new Properties(); p.load(stream);
					String method = p.getProperty("method", "ctm");
					boolean compact = method.equals("ctm_compact");
					if (!method.equals("ctm") && !compact) continue;
					// These predicates need world-dependent selection and must not become global rules.
					if (p.containsKey("biomes") || p.containsKey("heights") || p.containsKey("minHeight") || p.containsKey("maxHeight") || p.containsKey("name")) continue;
					if (!p.getProperty("faces", "all").equals("all")) continue;
					List<String> matches = matches(p, path);
					if (matches.isEmpty()) continue;
					List<String> names = expand(p.getProperty("tiles", compact ? "0-4" : "0-46"));
					if (names.size() < (compact ? 5 : GlassTiles.COUNT)) continue;
					BufferedImage[] images = new BufferedImage[compact ? names.size() : GlassTiles.COUNT];
					boolean complete = true;
					for (int i = 0; i < images.length; i++) {
						try (InputStream png = resources.open(resolve(path, names.get(i)))) { images[i] = ImageIO.read(png); }
						if (images[i] == null) { complete = false; break; }
					}
					if (complete && compact) images = compact(images, p);
					if (complete) for (String match : matches) if (!tiles.containsKey(match)) tiles.put(match, images);
				} catch (IOException | IllegalArgumentException ignored) {
					// A malformed pack rule falls back to generated tiles, as a missing vanilla texture would.
				}
			}
		} catch (IOException ignored) { return EMPTY; }
		return new GlassPacks(tiles);
	}

	/** Expand the compact format's four corner quadrants into the full atlas set. */
	static BufferedImage[] compact(BufferedImage[] tiles, Properties properties) {
		BufferedImage[] full = new BufferedImage[GlassTiles.COUNT];
		int w = tiles[0].getWidth(), h = tiles[0].getHeight();
		for (int i = 0; i < full.length; i++) {
			String replacement = properties.getProperty("ctm." + i);
			if (replacement != null) {
				int index = Integer.parseInt(replacement);
				if (index >= 0 && index < tiles.length) { full[i] = tiles[index]; continue; }
			}
			int mask = GlassTiles.mask(i);
			BufferedImage image = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
			for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
				boolean left = x < w / 2, top = y < h / 2;
				boolean horizontal = (mask & (left ? 1 : 16)) != 0;
				boolean vertical = (mask & (top ? 64 : 4)) != 0;
				boolean diagonal = (mask & (left ? (top ? 128 : 2) : (top ? 32 : 8))) != 0;
				int index = horizontal && vertical ? (diagonal ? 1 : 4) : horizontal ? 3 : vertical ? 2 : 0;
				BufferedImage tile = tiles[index];
				image.setRGB(x, y, tile.getRGB(x * tile.getWidth() / w, y * tile.getHeight() / h));
			}
			full[i] = image;
		}
		return full;
	}

	/** Legacy pack enumeration; files are ordered from highest to lowest priority. */
	public static GlassPacks files(final List<java.io.File> files) {
		return load(fileResources(files));
	}

	/** Enumerate file-backed active packs; callers may resolve streams through the resource manager. */
	public static Resources fileResources(final List<java.io.File> files) {
		return new Resources() {
			@Override public Collection<String> properties() throws IOException {
				java.util.Set<String> result = new java.util.LinkedHashSet<>();
				for (java.io.File file : files) {
					List<String> paths = new ArrayList<>();
					if (file.isDirectory()) {
						try (java.util.stream.Stream<java.nio.file.Path> walk = java.nio.file.Files.walk(file.toPath())) {
							walk.filter(java.nio.file.Files::isRegularFile).forEach(path -> paths.add(file.toPath().relativize(path).toString().replace('\\', '/')));
						}
					} else if (file.isFile()) {
						try (java.util.zip.ZipFile zip = new java.util.zip.ZipFile(file)) {
							java.util.Enumeration<? extends java.util.zip.ZipEntry> entries = zip.entries();
							while (entries.hasMoreElements()) paths.add(entries.nextElement().getName());
						}
					}
					Collections.sort(paths);
					for (String path : paths) if (path.startsWith("assets/") && path.endsWith(".properties") && (path.contains("/optifine/ctm/") || path.contains("/mcpatcher/ctm/"))) {
						int slash = path.indexOf('/', 7);
						result.add(path.substring(7, slash) + ":" + path.substring(slash + 1));
					}
				}
				return result;
			}
			@Override public InputStream open(String name) throws IOException {
				String path = "assets/" + name.replace(':', '/');
				if (path.contains("..")) throw new IOException("invalid resource path");
				for (java.io.File file : files) {
					if (file.isDirectory()) {
						java.io.File resource = new java.io.File(file, path);
						if (resource.isFile()) return new java.io.FileInputStream(resource);
					} else if (file.isFile()) {
						final java.util.zip.ZipFile zip = new java.util.zip.ZipFile(file);
						java.util.zip.ZipEntry entry = zip.getEntry(path);
						if (entry == null) { zip.close(); continue; }
						return new java.io.FilterInputStream(zip.getInputStream(entry)) {
							@Override public void close() throws IOException { try { super.close(); } finally { zip.close(); } }
						};
					}
				}
				throw new java.io.FileNotFoundException(name);
			}
		};
	}

	/** Return ARGB pixels for a preferred tile, or null when synthesis should be used. */
	public BufferedImage tile(String sprite, int tile) {
		BufferedImage[] images = tiles.get(canonical(sprite));
		return images == null ? null : images[tile];
	}

	public static String canonical(String sprite) {
		String name = sprite.substring(sprite.indexOf(':') + 1);
		name = name.substring(name.lastIndexOf('/') + 1).replace(".png", "");
		if (name.equals("glass_pane")) name = "glass";
		if (name.endsWith("_pane")) name = name.substring(0, name.length() - 5);
		if (name.startsWith("glass_")) name = name.substring(6) + "_stained_glass";
		name = name.replace("silver_stained_glass", "light_gray_stained_glass");
		return "minecraft:" + name;
	}

	private static List<String> matches(Properties p, String path) {
		List<String> result = new ArrayList<>();
		String blocks = p.getProperty("matchBlocks", "");
		String meta = p.getProperty("metadata", "0-15");
		for (String name : blocks.trim().split("\\s+")) {
			if (name.isEmpty()) continue;
			String bare = name.replace("minecraft:", "");
			if (bare.equals("20") || bare.equals("102")) result.add("minecraft:glass");
			else if (bare.matches("(?:95|160|stained_glass|stained_glass_pane)(?::[0-9-]+)?")) {
				String data = bare.contains(":") ? bare.substring(bare.indexOf(':') + 1) : meta;
				for (String n : expand(data)) {
					int i = Integer.parseInt(n);
					if (i >= 0 && i < COLORS.length) result.add("minecraft:" + COLORS[i] + "_stained_glass");
				}
			} else {
				String canonical = canonical(name);
				if (GlassTiles.glass(canonical)) result.add(canonical);
			}
		}
		for (String name : p.getProperty("matchTiles", "").trim().split("\\s+")) {
			if (!name.isEmpty() && GlassTiles.glass(canonical(name))) result.add(canonical(name));
		}
		if (blocks.isEmpty() && !p.containsKey("matchTiles")) {
			String name = canonical(path.substring(0, path.length() - ".properties".length()));
			if (GlassTiles.glass(name)) result.add(name);
		}
		return result;
	}

	static List<String> expand(String value) {
		List<String> out = new ArrayList<>();
		for (String token : value.trim().split("[ ,]+")) {
			if (token.matches("[0-9]+-[0-9]+")) {
				String[] ends = token.split("-");
				int start = Integer.parseInt(ends[0]), end = Integer.parseInt(ends[1]);
				if (start > 4096 || end > 4096 || end < start || (long) end - start > 4096) throw new IllegalArgumentException("tile range");
				for (long i = start; i <= end; i++) out.add(Long.toString(i));
			} else if (!token.isEmpty()) out.add(token);
		}
		return out;
	}

	static String resolve(String properties, String tile) {
		String namespace = properties.substring(0, properties.indexOf(':'));
		String directory = properties.substring(properties.indexOf(':') + 1, properties.lastIndexOf('/') + 1);
		String path;
		if (tile.startsWith("~/")) path = "optifine/" + tile.substring(2);
		else if (tile.startsWith("/")) path = tile.substring(1);
		else if (tile.startsWith("textures/") || tile.startsWith("optifine/") || tile.startsWith("mcpatcher/")) path = tile;
		else path = directory + (tile.startsWith("./") ? tile.substring(2) : tile);
		if (tile.contains(":")) { namespace = tile.substring(0, tile.indexOf(':')); path = tile.substring(tile.indexOf(':') + 1); }
		return namespace + ":" + path + (path.endsWith(".png") ? "" : ".png");
	}
}
