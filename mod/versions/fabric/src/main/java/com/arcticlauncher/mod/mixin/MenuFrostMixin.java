package com.arcticlauncher.mod.mixin;

//#if MC < 1.20.5
import com.arcticlauncher.client.feature.MenuBlur;
import net.minecraft.client.gui.screens.Screen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;

/** Menu Blur where the game has none (before 1.20.5): the dark gradient behind menus becomes a frosted veil with Arctic's strength. */
@Mixin(Screen.class)
abstract class MenuFrostMixin {
	@ModifyConstant(method = "renderBackground", constant = @Constant(intValue = -1072689136), require = 0)
	private int arctic$frostTop(int vanilla) {
		return MenuBlur.frost(vanilla);
	}

	@ModifyConstant(method = "renderBackground", constant = @Constant(intValue = -804253680), require = 0)
	private int arctic$frostBottom(int vanilla) {
		return MenuBlur.frost(vanilla);
	}
}
//#endif
