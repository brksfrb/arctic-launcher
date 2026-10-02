//#if MC >= 26.1
package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.GuiRecording;
import net.minecraft.client.renderer.state.gui.GuiElementRenderState;
import net.minecraft.client.renderer.state.gui.GuiItemRenderState;
import net.minecraft.client.renderer.state.gui.GuiRenderState;
import net.minecraft.client.renderer.state.gui.GuiTextRenderState;
import net.minecraft.client.renderer.state.gui.pip.PictureInPictureRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The GUI pieces the HUD hands the game, kept while it records them (GuiRecording). */
@Mixin(GuiRenderState.class)
abstract class GuiRenderStateRecordMixin {
	@Inject(method = "addItem", at = @At("HEAD"))
	private void arctic$item(GuiItemRenderState state, CallbackInfo ci) {
		GuiRecording.piece(state);
	}

	@Inject(method = "addText", at = @At("HEAD"))
	private void arctic$text(GuiTextRenderState state, CallbackInfo ci) {
		GuiRecording.piece(state);
	}

	@Inject(method = "addPicturesInPictureState", at = @At("HEAD"))
	private void arctic$picture(PictureInPictureRenderState state, CallbackInfo ci) {
		GuiRecording.piece(state);
	}

	@Inject(method = "addGuiElement", at = @At("HEAD"))
	private void arctic$element(GuiElementRenderState state, CallbackInfo ci) {
		GuiRecording.piece(state);
	}
}
//#endif
