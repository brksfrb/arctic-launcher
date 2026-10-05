//#if MC >= 26.1
package com.arcticlauncher.mod.startup;

import java.util.ArrayList;
import java.util.List;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.world.level.block.state.BlockState;

/**
 * The game works out a cache for each of its tens of thousands of block states, one after
 * another, while it starts. Each is independent of the others, so they are made on every
 * core at once. With mods the launcher didn't choose, they keep the game's own order.
 */
public final class StateCaches {
	private static final List<BlockState> PENDING = new ArrayList<>(32768);
	/** Mods that are known to leave this alone. */
	private static final java.util.Set<String> KNOWN = java.util.Set.of("minecraft", "java", "fabricloader", "mixinextras", "arctic",
			"sodium", "lithium", "ferritecore", "immediatelyfast", "entityculling", "modernfix", "polarium");

	private StateCaches() {}

	public static synchronized void defer(BlockState state) {
		PENDING.add(state);
	}

	public static synchronized void runAll() {
		List<BlockState> states = new ArrayList<>(PENDING);
		PENDING.clear();
		if (parallelIsSafe() && !"false".equals(System.getProperty("arctic.parallelStates"))) {
			states.parallelStream().forEach(BlockState::initCache);
		} else {
			states.forEach(BlockState::initCache);
		}
	}

	/** A fingerprint of what the caches say about every block state (to compare one order against another). */
	public static String digest() {
		java.security.MessageDigest sha;
		try {
			sha = java.security.MessageDigest.getInstance("SHA-256");
		} catch (java.security.NoSuchAlgorithmException e) {
			return "?";
		}
		var level = net.minecraft.world.level.EmptyBlockGetter.INSTANCE;
		var origin = net.minecraft.core.BlockPos.ZERO;
		for (net.minecraft.world.level.block.Block block : net.minecraft.core.registries.BuiltInRegistries.BLOCK) {
			for (BlockState state : block.getStateDefinition().getPossibleStates()) {
				StringBuilder line = new StringBuilder();
				line.append(state.getCollisionShape(level, origin).toAabbs());
				for (net.minecraft.core.Direction direction : net.minecraft.core.Direction.values()) {
					line.append(state.isFaceSturdy(level, origin, direction) ? '1' : '0');
				}
				line.append(state.canOcclude() ? '1' : '0').append(state.getLightEmission());
				sha.update(line.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
			}
		}
		return java.util.HexFormat.of().formatHex(sha.digest(), 0, 8);
	}

	private static boolean parallelIsSafe() {
		for (var mod : FabricLoader.getInstance().getAllMods()) {
			String id = mod.getMetadata().getId();
			if (!KNOWN.contains(id) && !id.startsWith("fabric-") && mod.getContainingMod().isEmpty()) {
				return false;
			}
		}
		return true;
	}
}
//#endif
