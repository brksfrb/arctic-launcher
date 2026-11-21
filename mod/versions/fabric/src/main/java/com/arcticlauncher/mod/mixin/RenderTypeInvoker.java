//#if MC >= 1.21.11 && MC < 26.1
package com.arcticlauncher.mod.mixin;

import net.minecraft.client.renderer.rendertype.RenderSetup;
import net.minecraft.client.renderer.rendertype.RenderType;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

@Mixin(RenderType.class)
public interface RenderTypeInvoker {
	@Invoker("create")
	static RenderType arctic$create(String name, RenderSetup setup) {
		throw new AssertionError();
	}
}
//#endif
