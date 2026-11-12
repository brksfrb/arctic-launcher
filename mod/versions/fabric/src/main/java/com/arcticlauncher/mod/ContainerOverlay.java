//#if MC >= 1.13
package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.ContainerPeek;
import com.arcticlauncher.client.feature.InventorySearch;
import net.minecraft.world.inventory.Slot;

/** What every container screen draws on top: the Inventory Search line and Shulker Peek. Shared by each screen hook. */
public final class ContainerOverlay {
	private ContainerOverlay() {}

	/** The search line and Shulker Peek over a container screen ({@code hovered}: the slot under the cursor, or null). */
	public static void draw(GfxImpl g, Slot hovered, int mouseX, int mouseY) {
		InventorySearch.seen();
		//#if MC >= 1.14
		com.arcticlauncher.mod.ContainerSort.hovered = hovered;
		//#endif
		if (ArcticClient.config() == null) {
			return;
		}
		InventorySearch.render(g, ArcticClient.config());
		if (ArcticClient.config().shulkerPeek && hovered != null && hovered.hasItem()) {
			ContainerPeek.hover(g, mouseX, mouseY, hovered.getItem());
		}
	}
}
//#endif
