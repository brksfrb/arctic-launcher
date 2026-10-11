package com.arcticlauncher.client.servers;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.DataOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class ServersFileTest {
	@TempDir
	Path dir;

	/** A servers.dat as the game writes it: three servers, the middle one hidden, an extra tag. */
	private File gameFile() throws Exception {
		File f = dir.resolve("servers.dat").toFile();
		try (DataOutputStream out = new DataOutputStream(new FileOutputStream(f))) {
			out.writeByte(10);
			out.writeUTF("");
			out.writeByte(9);
			out.writeUTF("servers");
			out.writeByte(10);
			out.writeInt(3);
			server(out, "Hypixel", "mc.hypixel.net", false, true);
			server(out, "direct", "1.2.3.4", true, false);
			server(out, "Home", "192.168.1.5:25570", false, false);
			out.writeByte(0);
		}
		return f;
	}

	private static void server(DataOutputStream out, String name, String ip, boolean hidden, boolean packs) throws Exception {
		out.writeByte(8);
		out.writeUTF("name");
		out.writeUTF(name);
		out.writeByte(8);
		out.writeUTF("ip");
		out.writeUTF(ip);
		if (hidden) {
			out.writeByte(1);
			out.writeUTF("hidden");
			out.writeByte(1);
		}
		if (packs) {
			out.writeByte(1);
			out.writeUTF("acceptTextures");
			out.writeByte(1);
		}
		// A tag from some newer version: kept as it is.
		out.writeByte(11);
		out.writeUTF("future");
		out.writeInt(2);
		out.writeInt(7);
		out.writeInt(9);
		out.writeByte(0);
	}

	private static List<String> names(ServersFile f) {
		List<String> out = new ArrayList<String>();
		for (int i = 0; i < f.size(); i++) {
			out.add(f.name(i));
		}
		return out;
	}

	@Test
	void readsTheShownServersAndTheirSettings() throws Exception {
		ServersFile f = new ServersFile(gameFile()).load();
		assertEquals(java.util.Arrays.asList("Hypixel", "Home"), names(f));
		assertEquals("192.168.1.5:25570", f.address(1));
		assertEquals(Boolean.TRUE, f.acceptsPacks(0));
		assertNull(f.acceptsPacks(1));
		assertNull(f.icon(0));
	}

	@Test
	void savingWithoutChangesKeepsEveryByte() throws Exception {
		File file = gameFile();
		byte[] before = Files.readAllBytes(file.toPath());
		new ServersFile(file).load().save();
		assertArrayEquals(before, Files.readAllBytes(file.toPath()));
		assertTrue(new File(file.getPath() + "_old").isFile());
	}

	@Test
	void movesAddsEditsAndRemovesAroundHiddenOnes() throws Exception {
		File file = gameFile();
		ServersFile f = new ServersFile(file).load();
		f.add("Arctic", "play.arctic.example");
		f.move(2, 0);
		f.save();
		f = new ServersFile(file).load();
		assertEquals(java.util.Arrays.asList("Arctic", "Hypixel", "Home"), names(f));
		f.move(0, 2);
		assertEquals(java.util.Arrays.asList("Hypixel", "Home", "Arctic"), names(f));
		f.edit(1, "Home 2", "192.168.1.6");
		f.remove(0);
		f.save();
		f = new ServersFile(file).load();
		assertEquals(java.util.Arrays.asList("Home 2", "Arctic"), names(f));
		assertEquals("192.168.1.6", f.address(0));
	}

	@Test
	void aMissingFileIsAnEmptyList() throws Exception {
		ServersFile f = new ServersFile(dir.resolve("none.dat").toFile()).load();
		assertEquals(0, f.size());
		f.add("New", "new.example");
		f.save();
		assertEquals(java.util.Arrays.asList("New"), names(new ServersFile(dir.resolve("none.dat").toFile()).load()));
	}
}
