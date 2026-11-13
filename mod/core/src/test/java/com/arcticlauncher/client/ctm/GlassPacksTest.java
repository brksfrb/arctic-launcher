package com.arcticlauncher.client.ctm;

import static org.junit.jupiter.api.Assertions.*;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.RandomAccessFile;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Collections;
import java.util.List;
import java.util.logging.Handler;
import java.util.logging.LogRecord;
import java.util.logging.Logger;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class GlassPacksTest {

	@TempDir Path directory;

	@Test void nullAndMissingRootsDoNotHideDirectoryOrZipResources() throws Exception {
		Path folder = directory.resolve("folder");
		Path rule = folder.resolve("assets/minecraft/optifine/ctm/glass/a.properties");
		Files.createDirectories(rule.getParent());
		Files.write(rule, new byte[]{42});
		Path archive = directory.resolve("pack.zip");
		try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(archive))) {
			zip.putNextEntry(new ZipEntry("assets/minecraft/mcpatcher/ctm/glass/b.properties"));
			zip.write(43);
			zip.closeEntry();
		}
		GlassPacks.Resources resources = GlassPacks.fileResources(Arrays.asList(
			null, directory.resolve("removed.zip").toFile(), folder.toFile(), null, archive.toFile()));
		String first = "minecraft:optifine/ctm/glass/a.properties";
		String second = "minecraft:mcpatcher/ctm/glass/b.properties";
		assertEquals(Arrays.asList(first, second), new ArrayList<>(resources.properties()));
		try (InputStream stream = resources.open(first)) { assertEquals(42, stream.read()); }
		try (InputStream stream = resources.open(second)) { assertEquals(43, stream.read()); }
		Files.delete(archive); // ZIP streams were closed, including on Windows.
	}

	@Test void corruptArchiveIsReportedOnceAndOtherPacksStillWork() throws Exception {
		Path corrupt = directory.resolve("broken.zip");
		Files.write(corrupt, new byte[]{1, 2, 3});
		Path rule = directory.resolve("assets/minecraft/optifine/ctm/glass/rule.properties");
		Files.createDirectories(rule.getParent());
		Files.write(rule, new byte[]{42});
		String name = "minecraft:optifine/ctm/glass/rule.properties";
		try (Warnings warnings = new Warnings()) {
			GlassPacks.Resources resources = GlassPacks.fileResources(Arrays.asList(corrupt.toFile(), directory.toFile()));
			assertEquals(Collections.singletonList(name), new ArrayList<>(resources.properties()));
			try (InputStream stream = resources.open(name)) { assertEquals(42, stream.read()); }
			resources.properties();
			assertEquals(1, warnings.records.size());
			assertTrue(warnings.records.get(0).getMessage().contains(corrupt.toString()));
		}
	}

	@Test void enumerationRuntimeFailureReturnsEmptyAndLogsPackOnce() {
		try (Warnings warnings = new Warnings()) {
			GlassPacks.Resources resources = new GlassPacks.Resources() {
				@Override public String description() { return "odd-server-pack.zip"; }
				@Override public Collection<String> properties() { throw new NullPointerException("no backing file"); }
				@Override public InputStream open(String name) { fail("enumeration failed"); return null; }
			};
			assertSame(GlassPacks.EMPTY, GlassPacks.load(resources));
			assertEquals(1, warnings.records.size());
			assertTrue(warnings.records.get(0).getMessage().contains("odd-server-pack.zip"));
		}
	}

	@Test void brokenZipEntryDoesNotHideReadableEntriesInTheSamePack() throws Exception {
		Path archive = directory.resolve("damaged-entry.zip");
		String bad = "minecraft:optifine/ctm/glass/bad.properties";
		String good = "minecraft:optifine/ctm/glass/good.properties";
		try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(archive))) {
			for (String name : Arrays.asList(bad, good)) {
				zip.putNextEntry(new ZipEntry("assets/" + name.replace(':', '/')));
				zip.write(42);
				zip.closeEntry();
			}
		}
		// Keep the central directory readable but break the first entry's local header.
		try (RandomAccessFile file = new RandomAccessFile(archive.toFile(), "rw")) { file.writeInt(0); }
		GlassPacks.Resources resources = GlassPacks.fileResources(Collections.singletonList(archive.toFile()));
		assertEquals(Arrays.asList(bad, good), new ArrayList<>(resources.properties()));
		assertThrows(IOException.class, () -> {
			try (InputStream stream = resources.open(bad)) { stream.read(); }
		});
		try (InputStream stream = resources.open(good)) { assertEquals(42, stream.read()); }
	}

	@Test void atlasBoundaryCatchesCheckedAndRuntimeFailures() {
		try (Warnings warnings = new Warnings()) {
			assertSame(GlassPacks.EMPTY, GlassPacks.safeLoad("legacy atlas", () -> { throw new IOException("bad pack"); }));
			assertSame(GlassPacks.EMPTY, GlassPacks.safeLoad("modern atlas", () -> { throw new IllegalStateException("bad pack"); }));
			assertEquals(2, warnings.records.size());
		}
	}

	@Test void propertyAndTileFailuresSkipOnlyTheirRule() throws Exception {
		for (boolean tileFailure : new boolean[]{false, true}) {
			for (boolean runtimeFailure : new boolean[]{false, true}) {
				assertBadRuleSkipped(tileFailure, runtimeFailure);
			}
		}
	}

	private void assertBadRuleSkipped(boolean tileFailure, boolean runtimeFailure) throws Exception {
		String bad = "minecraft:optifine/ctm/bad/glass.properties";
		String good = "minecraft:optifine/ctm/good/glass.properties";
		byte[] rule = "matchBlocks=glass\ntiles=0-46\n".getBytes(StandardCharsets.ISO_8859_1);
		BufferedImage image = new BufferedImage(1, 1, BufferedImage.TYPE_INT_ARGB);
		image.setRGB(0, 0, 0xffabcdef);
		ByteArrayOutputStream output = new ByteArrayOutputStream();
		ImageIO.write(image, "png", output);
		List<String> closed = new ArrayList<>();
		GlassPacks.Resources resources = new GlassPacks.Resources() {
			@Override public String description() { return "broken-art.zip"; }
			@Override public Collection<String> properties() { return Arrays.asList(bad, good); }
			@Override public InputStream open(String name) throws IOException {
				if (name.equals(tileFailure ? "minecraft:optifine/ctm/bad/0.png" : bad)) {
					if (runtimeFailure) throw new IllegalStateException("broken resource");
					throw new IOException("broken resource");
				}
				return new ByteArrayInputStream(name.endsWith(".properties") ? rule : output.toByteArray()) {
					@Override public void close() throws IOException { closed.add(name); super.close(); }
				};
			}
		};
		try (Warnings warnings = new Warnings()) {
			GlassPacks packs = GlassPacks.load(resources);
			assertEquals(0xffabcdef, packs.tile("minecraft:glass", 26).getRGB(0, 0));
			assertTrue(closed.contains(good));
			if (tileFailure) assertTrue(closed.contains(bad));
			assertEquals(1, warnings.records.size());
			LogRecord warning = warnings.records.get(0);
			assertTrue(warning.getMessage().contains(bad));
			assertTrue(warning.getMessage().contains("broken-art.zip"));
			if (tileFailure) assertTrue(warning.getThrown().getMessage().contains("bad/0.png"));
		}
	}

	private static final class Warnings extends Handler implements AutoCloseable {
		final List<LogRecord> records = new ArrayList<>();
		final Logger logger = Logger.getLogger(GlassPacks.class.getName());
		Warnings() { logger.addHandler(this); }
		@Override public void publish(LogRecord record) { records.add(record); }
		@Override public void flush() {}
		@Override public void close() { logger.removeHandler(this); }
	}
}
