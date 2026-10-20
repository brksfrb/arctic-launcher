//#if MC < 1.14
package com.arcticlauncher.mod.mixin;

import java.util.List;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.widget.ButtonWidget;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** 1.13 has no list of a screen's widgets for the Arctic buttons to be placed among: its buttons are all there is. */
@Mixin(Screen.class)
public interface ScreenButtonsAccess {
	@Accessor("buttons")
	List<ButtonWidget> arctic$buttons();
}
//#endif
