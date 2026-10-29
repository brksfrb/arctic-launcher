package com.arcticlauncher.mod;

//#if MC >= 1.15 && MC < 1.21.9
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
//#endif
//#if MC >= 1.21.11
import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.Minecraft;
//#endif

/**
 * The Hitboxes mod before Minecraft 26.1: the game's own F3+B flag is set while the switch is on, and put
 * back when it is turned off (but only if this switched it on, so the player's own F3+B stays theirs).
 * On 1.21.11 and later the game's debug list is asked to re-read its entries when a switch changes.
 */
public final class HitboxSync {
	//#if MC >= 1.15 && MC < 1.21.9
	private static boolean ours;
	//#endif
	//#if MC >= 1.21.11
	private static boolean lastHitboxes;
	private static boolean lastChunkBorders;
	//#endif

	private HitboxSync() {}

	public static void tick() {
		//#if MC >= 1.15 && MC < 1.21.9
		if (ArcticClient.config() == null) {
			return;
		}
		EntityRenderDispatcher dispatcher = Minecraft.getInstance().getEntityRenderDispatcher();
		boolean want = ArcticClient.config().hitboxes;
		if (want && !dispatcher.shouldRenderHitBoxes()) {
			dispatcher.setRenderHitBoxes(true);
			ours = true;
		} else if (!want && ours) {
			dispatcher.setRenderHitBoxes(false);
			ours = false;
		}
		//#endif
		//#if MC >= 1.21.11
		if (ArcticClient.config() == null) {
			return;
		}
		boolean hitboxes = ArcticClient.config().hitboxes;
		boolean chunkBorders = ArcticClient.config().chunkBorders;
		if (hitboxes != lastHitboxes || chunkBorders != lastChunkBorders) {
			lastHitboxes = hitboxes;
			lastChunkBorders = chunkBorders;
			((DebugListRebuild) (Object) Minecraft.getInstance().debugEntries).arctic$rebuild();
		}
		//#endif
	}
}
