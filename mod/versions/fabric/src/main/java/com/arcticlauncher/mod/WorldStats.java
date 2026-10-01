package com.arcticlauncher.mod;

import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.LightLayer;

/** What the extra HUD widgets read: light, items, food, the resource pack. */
final class WorldStats {
	private WorldStats() {}

	static int[] light() {
		Minecraft mc = Minecraft.getInstance();
		LocalPlayer p = mc.player;
		if (p == null || mc.level == null) {
			return null;
		}
		BlockPos pos = Compat.blockPos(p);
		return new int[] {mc.level.getBrightness(LightLayer.BLOCK, pos), mc.level.getBrightness(LightLayer.SKY, pos)};
	}

	static int countItems(String idPart) {
		LocalPlayer p = Minecraft.getInstance().player;
		if (p == null) {
			return -1;
		}
		Inventory inventory = Compat.inventory(p);
		int n = 0;
		for (int i = 0; i < inventory.getContainerSize(); i++) {
			ItemStack stack = inventory.getItem(i);
			if (!stack.isEmpty() && BuiltInRegistries.ITEM.getKey(stack.getItem()).getPath().contains(idPart)) {
				n += stack.getCount();
			}
		}
		return n;
	}

	/** Arrows by kind (their name tells tipped kinds apart): {stack, count}. */
	static java.util.List<Object[]> arrows() {
		LocalPlayer p = Minecraft.getInstance().player;
		if (p == null) {
			return java.util.Collections.emptyList();
		}
		Inventory inventory = Compat.inventory(p);
		java.util.Map<String, Object[]> kinds = new java.util.LinkedHashMap<String, Object[]>();
		for (int i = 0; i < inventory.getContainerSize(); i++) {
			ItemStack stack = inventory.getItem(i);
			if (stack.isEmpty() || !BuiltInRegistries.ITEM.getKey(stack.getItem()).getPath().endsWith("arrow")) {
				continue;
			}
			String kind = stack.getHoverName().getString();
			Object[] row = kinds.get(kind);
			if (row == null) {
				kinds.put(kind, new Object[] {stack, stack.getCount()});
			} else {
				row[1] = (Integer) row[1] + stack.getCount();
			}
		}
		return new java.util.ArrayList<Object[]>(kinds.values());
	}

	static float[] food() {
		LocalPlayer p = Minecraft.getInstance().player;
		if (p == null) {
			return null;
		}
		return new float[] {p.getFoodData().getFoodLevel(), p.getFoodData().getSaturationLevel()};
	}

	/** The highest resource pack you added (not the built-in ones). */
	static String resourcePack() {
		String top = null;
		for (net.minecraft.server.packs.repository.Pack pack : selectedPacks(Minecraft.getInstance().getResourcePackRepository())) {
			String id = pack.getId();
			if (id.startsWith("file/") || id.startsWith("server/") || id.startsWith("world/")) {
				top = pack.getTitle().getString();
			}
		}
		return top;
	}

	/** The server address, or "sp:" + the singleplayer world's name; null outside a world. */
	static String worldKey() {
		Minecraft mc = Minecraft.getInstance();
		if (mc.level == null) {
			return null;
		}
		if (mc.getSingleplayerServer() != null) {
			//#if MC >= 1.16
			return "sp:" + mc.getSingleplayerServer().getWorldData().getLevelName();
			//#else
			return "sp:" + mc.getSingleplayerServer().getLevelName();
			//#endif
		}
		net.minecraft.client.multiplayer.ServerData server = mc.getCurrentServer();
		return server == null ? "unknown" : server.ip.toLowerCase(java.util.Locale.ROOT);
	}

	static String dimension() {
		Minecraft mc = Minecraft.getInstance();
		if (mc.level == null) {
			return "overworld";
		}
		//#if MC >= 1.21.11
		return mc.level.dimension().identifier().getPath();
		//#elif MC >= 1.16
		return mc.level.dimension().location().getPath();
		//#else
		return net.minecraft.world.level.dimension.DimensionType.getName(mc.level.getDimension().getType()).getPath();
		//#endif
	}

	/** Turn on a pack from the resource pack folder, on top of the others, and reload. */
	static void enablePack(String fileName) {
		Minecraft mc = Minecraft.getInstance();
		net.minecraft.server.packs.repository.PackRepository repo = mc.getResourcePackRepository();
		repo.reload();
		String id = "file/" + fileName;
		if (!isAvailable(repo, id)) {
			ArcticMod.LOG.warn("pack {} isn't available after the download", id);
			return;
		}
		java.util.List<String> selected = selectedIds(repo);
		selected.remove(id);
		// Below Arctic's own packs (the smooth font), above everything else.
		int at = selected.size();
		while (at > 0 && selected.get(at - 1).startsWith("arctic")) {
			at--;
		}
		selected.add(at, id);
		select(repo, selected);
		applyPacks(mc, repo);
	}

	/** Every pack but Arctic's own: on ones top first, then the rest by name. */
	static java.util.List<com.arcticlauncher.client.packs.PackInfo> packs(java.io.File folder) {
		net.minecraft.server.packs.repository.PackRepository repo = Minecraft.getInstance().getResourcePackRepository();
		repo.reload();
		java.util.List<String> selected = selectedIds(repo);
		java.util.List<com.arcticlauncher.client.packs.PackInfo> on = new java.util.ArrayList<>();
		java.util.List<com.arcticlauncher.client.packs.PackInfo> off = new java.util.ArrayList<>();
		for (net.minecraft.server.packs.repository.Pack p : availablePacks(repo)) {
			String id = p.getId();
			if (id.startsWith("arctic")) {
				continue;
			}
			java.io.File file = id.startsWith("file/") ? new java.io.File(folder, id.substring("file/".length())) : null;
			com.arcticlauncher.client.packs.PackInfo info = new com.arcticlauncher.client.packs.PackInfo(id, p.getTitle().getString(),
					p.getDescription().getString(), selected.contains(id), p.isRequired(), file);
			(info.enabled ? on : off).add(info);
		}
		// Selected ids go bottom to top; show the top first.
		on.sort(java.util.Comparator.comparingInt((com.arcticlauncher.client.packs.PackInfo i) -> -selected.indexOf(i.id)));
		off.sort(java.util.Comparator.comparing((com.arcticlauncher.client.packs.PackInfo i) -> i.title.toLowerCase(java.util.Locale.ROOT)));
		on.addAll(off);
		return on;
	}

	/** Exactly these packs on (top first), keeping required and Arctic's own ones. */
	static void setPacks(java.util.List<String> enabledTopFirst) {
		Minecraft mc = Minecraft.getInstance();
		net.minecraft.server.packs.repository.PackRepository repo = mc.getResourcePackRepository();
		java.util.List<String> bottomFirst = new java.util.ArrayList<>();
		java.util.List<String> arctic = new java.util.ArrayList<>();
		for (String id : selectedIds(repo)) {
			net.minecraft.server.packs.repository.Pack p = repo.getPack(id);
			if (id.startsWith("arctic")) {
				arctic.add(id);
			} else if (p != null && p.isRequired() && !enabledTopFirst.contains(id)) {
				bottomFirst.add(id);
			}
		}
		for (int i = enabledTopFirst.size() - 1; i >= 0; i--) {
			String id = enabledTopFirst.get(i);
			if (isAvailable(repo, id) && !bottomFirst.contains(id)) {
				bottomFirst.add(id);
			}
		}
		bottomFirst.addAll(arctic);
		select(repo, bottomFirst);
		applyPacks(mc, repo);
	}

	/** Delete the saves whose folder names start with {@code prefix}. */
	static void deleteWorlds(java.io.File saves, String prefix) {
		java.io.File[] dirs = saves.listFiles();
		if (dirs == null) {
			return;
		}
		for (java.io.File dir : dirs) {
			if (!dir.isDirectory() || !dir.getName().startsWith(prefix)) {
				continue;
			}
			try (java.util.stream.Stream<java.nio.file.Path> files = java.nio.file.Files.walk(dir.toPath())) {
				files.sorted(java.util.Comparator.reverseOrder()).forEach(f -> f.toFile().delete());
			} catch (java.io.IOException e) {
				ArcticMod.LOG.warn("couldn't delete {}: {}", dir.getName(), e.toString());
			}
		}
	}

	static boolean dead() {
		LocalPlayer p = Minecraft.getInstance().player;
		//#if MC >= 1.16
		return p != null && p.isDeadOrDying();
		//#else
		return p != null && p.getHealth() <= 0;
		//#endif
	}

	static void sendChat(String text) {
		Compat.sendChat(text);
	}

	/** Save the chosen packs to the options and reload. */
	private static void applyPacks(Minecraft mc, net.minecraft.server.packs.repository.PackRepository repo) {
		//#if MC >= 1.19.4
		mc.options.updateResourcePacks(repo);
		//#else
		mc.options.resourcePacks.clear();
		mc.options.incompatibleResourcePacks.clear();
		for (net.minecraft.server.packs.repository.Pack pack : selectedPacks(repo)) {
			if (!pack.isFixedPosition()) {
				mc.options.resourcePacks.add(pack.getId());
				if (!pack.getCompatibility().isCompatible()) {
					mc.options.incompatibleResourcePacks.add(pack.getId());
				}
			}
		}
		mc.options.save();
		mc.reloadResourcePacks();
		//#endif
	}

	// ---- The pack repository (generic, without id lookups, before 1.16) ----------------

	@SuppressWarnings({"rawtypes", "unchecked"})
	private static java.util.Collection<? extends net.minecraft.server.packs.repository.Pack> selectedPacks(
			net.minecraft.server.packs.repository.PackRepository repo) {
		//#if MC >= 1.16
		return repo.getSelectedPacks();
		//#else
		return repo.getSelected();
		//#endif
	}

	@SuppressWarnings({"rawtypes", "unchecked"})
	private static java.util.Collection<? extends net.minecraft.server.packs.repository.Pack> availablePacks(
			net.minecraft.server.packs.repository.PackRepository repo) {
		//#if MC >= 1.16
		return repo.getAvailablePacks();
		//#else
		return repo.getAvailable();
		//#endif
	}

	/** Selected pack ids, bottom first. */
	@SuppressWarnings("rawtypes")
	private static java.util.List<String> selectedIds(net.minecraft.server.packs.repository.PackRepository repo) {
		//#if MC >= 1.16
		return new java.util.ArrayList<>(repo.getSelectedIds());
		//#else
		java.util.List<String> ids = new java.util.ArrayList<>();
		for (net.minecraft.server.packs.repository.Pack p : selectedPacks(repo)) {
			ids.add(p.getId());
		}
		return ids;
		//#endif
	}

	@SuppressWarnings("rawtypes")
	private static boolean isAvailable(net.minecraft.server.packs.repository.PackRepository repo, String id) {
		//#if MC >= 1.16
		return repo.isAvailable(id);
		//#else
		return repo.getPack(id) != null;
		//#endif
	}

	@SuppressWarnings({"rawtypes", "unchecked"})
	private static void select(net.minecraft.server.packs.repository.PackRepository repo, java.util.List<String> ids) {
		//#if MC >= 1.16
		repo.setSelected(ids);
		//#else
		java.util.List packs = new java.util.ArrayList();
		for (String id : ids) {
			Object p = repo.getPack(id);
			if (p != null) {
				packs.add(p);
			}
		}
		repo.setSelected(packs);
		//#endif
	}
}
