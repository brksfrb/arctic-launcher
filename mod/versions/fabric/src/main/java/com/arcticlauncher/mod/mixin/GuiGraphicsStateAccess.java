//#if MC >= 26.1
package com.arcticlauncher.mod.mixin;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.renderer.state.gui.GuiRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** Where a GUI draw's pieces go, to hand recorded ones in again. */
@Mixin(GuiGraphicsExtractor.class)
public interface GuiGraphicsStateAccess {
	@Accessor("guiRenderState")
	GuiRenderState arctic$guiRenderState();
}
//#endif
