package com.arcticlauncher.mod.mixin;
import net.minecraft.client.gui.components.AbstractWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;
@Mixin(AbstractWidget.class)
public interface WidgetAlphaAccess {
	@Accessor("alpha") float arctic$alpha();
}
