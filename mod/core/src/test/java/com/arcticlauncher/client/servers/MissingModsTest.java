package com.arcticlauncher.client.servers;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class MissingModsTest {
	private static List<String> ids(MissingMods.Info info) {
		List<String> out = new ArrayList<String>();
		for (MissingMods.Entry e : info.mods) {
			out.add(e.id);
		}
		return out;
	}

	@Test
	void fabricRegistrySyncNamesTheMods() {
		MissingMods.Info info = MissingMods.parse("Received a registry sync packet with missing entries!\n"
				+ "Please install the missing mods.\n"
				+ "minecraft:block: create:andesite_alloy, create:shaft, farmersdelight:stove\n"
				+ "minecraft:item: minecraft:stone, c:ingots");
		assertTrue(info.aboutMods);
		assertEquals(java.util.Arrays.asList("create", "farmersdelight"), ids(info));
		assertNull(info.loader);
	}

	@Test
	void forgeModRejectionsGiveIdsAndVersions() {
		MissingMods.Info info = MissingMods.parse("Mod rejections [(examplemod,1.0.2), (JEI,3.2.1)]");
		assertTrue(info.aboutMods);
		assertEquals(java.util.Arrays.asList("examplemod", "jei"), ids(info));
		assertEquals("1.0.2", info.mods.get(0).version);
		assertEquals("3.2.1", info.mods.get(1).version);

		MissingMods.Info map = MissingMods.parse("§cMod rejections {ironchest=6.0.62, journeymap=5.1.4}");
		assertEquals(java.util.Arrays.asList("ironchest", "journeymap"), ids(map));
		assertEquals("6.0.62", map.mods.get(0).version);
	}

	@Test
	void listedMissingModsAreRead() {
		MissingMods.Info info = MissingMods.parse("You are missing the following mods: sodium 0.6.1, lithium, iris (1.8.0)");
		assertEquals(java.util.Arrays.asList("sodium", "lithium", "iris"), ids(info));
		assertEquals("0.6.1", info.mods.get(0).version);
		assertNull(info.mods.get(1).version);
		assertEquals("1.8.0", info.mods.get(2).version);
	}

	@Test
	void aLoaderTheServerNeedsIsNamed() {
		MissingMods.Info neo = MissingMods.parse("You are trying to connect to a server that is running NeoForge, but you are not. "
				+ "Please install NeoForge Version: 21.1.77 to connect to this server.");
		assertTrue(neo.aboutMods);
		assertEquals("neoforge", neo.loader);
		assertTrue(neo.mods.isEmpty());

		MissingMods.Info forge = MissingMods.parse("This server has mods that require Forge to be installed on the client. Contact your server admin for more details.");
		assertEquals("forge", forge.loader);
		MissingMods.Info fml = MissingMods.parse("This server requires FML/Forge to be installed. Contact your server admin for more details.");
		assertEquals("forge", fml.loader);
	}

	@Test
	void genericModRefusalsAreRecognisedWithoutNames() {
		MissingMods.Info info = MissingMods.parse("Incompatible mod list: please use the server's modpack");
		assertTrue(info.aboutMods);
		assertTrue(info.mods.isEmpty());
		assertTrue(MissingMods.parse("Mismatched mod channel list").aboutMods);
	}

	@Test
	void otherRefusalsAreNotAboutMods() {
		for (String reason : new String[] {
				"You are banned from this server.",
				"You are not whitelisted on this server!",
				"Outdated client! Please use 1.21.4",
				"Incompatible client! Please use 1.21.11",
				"Internal Exception: io.netty.handler.timeout.ReadTimeoutException",
				"Disconnected",
				"",
				"Server closed",
				"Visit https://example.net:443/rules for the rules"}) {
			MissingMods.Info info = MissingMods.parse(reason);
			assertFalse(info.aboutMods, reason);
			assertTrue(info.mods.isEmpty(), reason);
		}
		assertFalse(MissingMods.parse(null).aboutMods);
	}

	@Test
	void namesAreCleanedAndCapped() {
		StringBuilder many = new StringBuilder("Received a registry sync packet with missing entries: ");
		for (int i = 0; i < 200; i++) {
			many.append("mod").append(i).append(":thing, ");
		}
		MissingMods.Info info = MissingMods.parse(many.toString());
		assertEquals(MissingMods.MAX_MODS, info.mods.size());
		MissingMods.Info odd = MissingMods.parse("Missing mods: ../evil, good_mod, a, minecraft");
		assertEquals(java.util.Arrays.asList("good_mod"), ids(odd));
	}

	@Test
	void theRequestCarriesWhatTheServerSaid() {
		MissingMods.Info info = MissingMods.parse("Mod rejections [(examplemod,1.0)]");
		String body = MissingModsLink.body("token", "play.example.net", info, "Mod rejections [(examplemod,1.0)]");
		JsonObject o = new Gson().fromJson(body, JsonObject.class);
		assertEquals("token", o.get("launch").getAsString());
		assertEquals("play.example.net", o.get("server").getAsString());
		assertEquals("examplemod", o.getAsJsonArray("mods").get(0).getAsJsonObject().get("id").getAsString());
		assertEquals("1.0", o.getAsJsonArray("mods").get(0).getAsJsonObject().get("version").getAsString());
		assertFalse(o.has("loader"));
	}

	@Test
	void theLauncherStateIsRead() {
		MissingModsLink link = new MissingModsLink(0, null, null);
		assertFalse(link.available());
		assertEquals("offline", link.poll().state);
		MissingModsLink.State s = link.parse("{\"state\":\"installed\",\"installed\":2,\"not_found\":[\"x\"],\"message\":null}");
		assertEquals("installed", s.state);
		assertEquals(2, s.installed);
		assertEquals(java.util.Arrays.asList("x"), s.notFound);
		assertNull(s.message);
	}
}
