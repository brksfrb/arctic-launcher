package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.mod.GuiRecording;
import com.arcticlauncher.mod.ScaledInter;
import net.minecraft.client.gui.font.FontManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Recorded HUD text must be shaped again after the font textures have been replaced. */
@Mixin(FontManager.class)
abstract class FontReloadMixin {
	@Inject(method = "apply", at = @At("RETURN"))
	private void arctic$fontsReloaded(CallbackInfo ci) {
		ScaledInter.clear();
		GuiRecording.fontsReloaded();
	}
}
//#endif
