package com.arcticlauncher.mod.mixin;

import net.minecraft.client.MouseHandler;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** The game's own mouse handling, for a controller's pointer: clicks and scrolls go where real ones go. */
@Mixin(MouseHandler.class)
public interface MouseHandlerInvoker {
	//#if MC >= 1.21.9
	@Invoker("onButton")
	void arctic$button(long window, net.minecraft.client.input.MouseButtonInfo button, int action);
	//#else
	@Invoker("onPress")
	void arctic$press(long window, int button, int action, int mods);
	//#endif

	//#if MC < 26.3
	@Invoker("onMove")
	void arctic$move(long window, double x, double y);
	//#endif

	@Invoker("onScroll")
	void arctic$scroll(long window, double x, double y);
}
