package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.gui.components.Button;
import net.minecraft.network.chat.Component;

/** The "Arctic" button on the pause screen and the Vanilla title screen. */
public final class ArcticButton {
	private ArcticButton() {}

	public static Button create() {
		//#if MC >= 1.19.3
		return Button.builder(com.arcticlauncher.mod.Compat.literal("Arctic"), b -> ArcticClient.platform().openPage(ArcticClient.arcticMenu()))
				.bounds(4, 4, 64, 20)
				.build();
		//#elif MC >= 1.16
		return new Button(4, 4, 64, 20, com.arcticlauncher.mod.Compat.literal("Arctic"), b -> ArcticClient.platform().openPage(ArcticClient.arcticMenu()));
		//#else
		// Before 1.16 button labels were plain strings.
		return new Button(4, 4, 64, 20, "Arctic", b -> ArcticClient.platform().openPage(ArcticClient.arcticMenu()));
		//#endif
	}
}
