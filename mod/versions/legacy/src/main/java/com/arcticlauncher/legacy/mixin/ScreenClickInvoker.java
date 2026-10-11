package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.gui.screen.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** A controller's clicks, straight to the screen (LWJGL 2's mouse can't be fed events). */
@Mixin(Screen.class)
public interface ScreenClickInvoker {
	@Invoker("mouseClicked")
	void arctic$clicked(int x, int y, int button) throws java.io.IOException;

	@Invoker("mouseReleased")
	void arctic$released(int x, int y, int button);
}
