package com.arcticlauncher.client.packs;

import static org.junit.jupiter.api.Assertions.*;

import java.io.IOException;
import org.junit.jupiter.api.Test;
import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

class PackBrowserTest {
	private final Gson gson = new Gson();

	@Test
	void shaderFiltersCombineLoaderVersionStyleAndPerformance() {
		PackBrowser.Filters filters = new PackBrowser.Filters("updated", true, "realistic", "low");
		JsonArray facets = gson.fromJson(filters.facets(PackBrowser.Kind.SHADER, "1.21.11"), JsonArray.class);
		assertEquals(5, facets.size());
		assertEquals("project_type:shader", facets.get(0).getAsJsonArray().get(0).getAsString());
		assertEquals("categories:iris", facets.get(1).getAsJsonArray().get(0).getAsString());
		assertEquals("versions:1.21.11", facets.get(2).getAsJsonArray().get(0).getAsString());
		assertEquals("categories:realistic", facets.get(3).getAsJsonArray().get(0).getAsString());
		assertEquals("categories:low", facets.get(4).getAsJsonArray().get(0).getAsString());
	}

	@Test
	void allVersionsDropsOnlyTheVersionConstraint() {
		String facets = new PackBrowser.Filters("downloads", false, "combat", "32x")
				.facets(PackBrowser.Kind.RESOURCE, "1.21.11");
		assertFalse(facets.contains("versions:"));
		assertTrue(facets.contains("project_type:resourcepack"));
		assertTrue(facets.contains("categories:combat"));
		assertTrue(facets.contains("categories:32x"));
		assertFalse(facets.contains("iris"));
	}

	@Test
	void incompatibleVersionsAndOptifineOnlyReleasesAreNeverInstalledAsIrisPacks() throws Exception {
		JsonArray versions = gson.fromJson("["
				+ "{game_versions:['1.21.10'],loaders:['iris'],files:[{filename:'wrong-game.zip',primary:true}]},"
				+ "{game_versions:['1.21.11'],loaders:['optifine'],files:[{filename:'wrong-loader.zip',primary:true}]},"
				+ "{game_versions:['1.21.11'],loaders:['iris'],files:[{filename:'extra.zip',primary:false},{filename:'right.zip',primary:true}]}]",
				JsonArray.class);
		JsonObject file = PackBrowser.compatibleFile(versions, "1.21.11", PackBrowser.Kind.SHADER);
		assertEquals("right.zip", file.get("filename").getAsString());
		assertThrows(IOException.class, () -> PackBrowser.compatibleFile(versions, "1.20.1", PackBrowser.Kind.SHADER));
	}

	@Test
	void resourcePacksDoNotRequireAShaderLoader() throws Exception {
		JsonArray versions = gson.fromJson("[{game_versions:['1.21.11'],loaders:['minecraft'],files:[{filename:'pack.zip'}]}]", JsonArray.class);
		assertEquals("pack.zip", PackBrowser.compatibleFile(versions, "1.21.11", PackBrowser.Kind.RESOURCE).get("filename").getAsString());
		assertThrows(IOException.class, () -> PackBrowser.compatibleFile(versions, "1.21.11", PackBrowser.Kind.SHADER));
	}

	@Test
	void allVersionSearchResultsExposeCompatibilityForTheInstallButton() {
		JsonObject result = gson.fromJson("{project_id:'example',versions:['1.21.11']}", JsonObject.class);
		assertTrue(new PackBrowser.Pack(result, "1.21.11").compatible);
		assertFalse(new PackBrowser.Pack(result, "1.20.1").compatible);
	}
}
