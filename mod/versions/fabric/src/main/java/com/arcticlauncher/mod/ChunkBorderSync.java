package com.arcticlauncher.mod;

//#if MC >= 1.14 && MC < 1.21.9
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.debug.DebugRenderer;
//#endif

/**
 * The Chunk Borders mod before Minecraft 1.21.9: the game's own F3+G is switched on while Arctic's switch is on, and
 * back off when it is turned off (only if this switched it on, so the player's own F3+G stays theirs). From 1.21.9
 * the debug entry list says so instead (see DebugEntriesMixin).
 */
public final class ChunkBorderSync {
	//#if MC >= 1.14 && MC < 1.21.9
	private static boolean ours;
	//#endif

	private ChunkBorderSync() {}

	public static void tick() {
		//#if MC >= 1.14 && MC < 1.21.9
		if (ArcticClient.config() == null) {
			return;
		}
		boolean want = ArcticClient.config().chunkBorders;
		if (!want && !ours) {
			return;
		}
		DebugRenderer debug = Minecraft.getInstance().debugRenderer;
		// The game only offers "switch" (it returns the new state): switching twice reads the state and changes nothing.
		boolean shown = !debug.switchRenderChunkborder();
		debug.switchRenderChunkborder();
		if (want && !shown) {
			debug.switchRenderChunkborder();
			ours = true;
		} else if (!want && ours) {
			if (shown) {
				debug.switchRenderChunkborder();
			}
			ours = false;
		}
		//#endif
	}
}
