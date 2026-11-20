package com.arcticlauncher.legacy;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.awt.image.BufferedImage;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.stream.Collectors;
import net.minecraft.client.option.GameOptions;
import net.minecraft.client.resource.PackFormatMetadataSerializer;
import net.minecraft.client.resource.ResourcePackLoader;
import net.minecraft.client.resource.ResourcePackMetadata;
import net.minecraft.resource.DirectoryResourcePack;
import net.minecraft.util.MetadataSerializer;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class LegacyPackOptionsTest {
	@TempDir Path directory;

	@Test void selectedPacksAndTheirCompatibilitySurviveReopeningTheRepository() throws IOException {
		List<String> names = Arrays.asList("two", "one", "future", "three");
		int[] formats = {2, 1, 999, 3};
		Path folder = Files.createDirectory(directory.resolve("resourcepacks"));
		for (int i = 0; i < names.size(); i++) {
			Path pack = Files.createDirectory(folder.resolve(names.get(i)));
			String metadata = "{\"pack\":{\"description\":\"test\",\"pack_format\":" + formats[i] + "}}";
			Files.write(pack.resolve("pack.mcmeta"), metadata.getBytes(StandardCharsets.UTF_8));
		}

		GameOptions options = new GameOptions();
		options.resourcePacks.addAll(names);
		ResourcePackLoader vanilla = open(folder, options);
		List<String> compatible = namesOf(vanilla.getSelectedResourcePacks());
		assertEquals(1, compatible.size());

		List<ResourcePackLoader.Entry> ordered = new ArrayList<>();
		for (String name : names) {
			ordered.add(vanilla.getAvailableResourcePacks().stream()
					.filter(pack -> name.equals(pack.getName())).findFirst().get());
		}
		options.incompatibleResourcePacks.add("removed");
		assertEquals(names, namesOf(LegacyPlatform.updateResourcePackOptions(options, ordered)));
		List<String> incompatible = new ArrayList<>(names);
		incompatible.removeAll(compatible);
		assertEquals(incompatible, options.incompatibleResourcePacks);
		assertEquals(names, namesOf(open(folder, options).getSelectedResourcePacks()));
	}

	private ResourcePackLoader open(Path folder, GameOptions options) {
		MetadataSerializer metadata = new MetadataSerializer();
		metadata.register(new PackFormatMetadataSerializer(), ResourcePackMetadata.class);
		// Local packs have no pack.png. Supply their fallback icon without initializing OpenGL on 1.8.9.
		DirectoryResourcePack defaultPack = new DirectoryResourcePack(directory.toFile()) {
			@Override public BufferedImage getIcon() { return new BufferedImage(1, 1, BufferedImage.TYPE_INT_ARGB); }
		};
		return new ResourcePackLoader(folder.toFile(), directory.resolve("server-packs").toFile(), defaultPack, metadata, options);
	}

	private static List<String> namesOf(List<ResourcePackLoader.Entry> packs) {
		return packs.stream().map(ResourcePackLoader.Entry::getName).collect(Collectors.toList());
	}
}
