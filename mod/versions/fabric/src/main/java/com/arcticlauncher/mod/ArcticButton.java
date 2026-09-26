package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.gui.components.Button;
import net.minecraft.network.chat.Component;

/** The "Arctic" button on the pause screen and the Classic title screen. */
public final class ArcticButton {
	private ArcticButton() {}

	public static Button create() {
		return Button.builder(Component.literal("Arctic"), b -> ArcticClient.platform().openPage(ArcticClient.arcticMenu()))
				.bounds(4, 4, 64, 20)
				.build();
	}
}
