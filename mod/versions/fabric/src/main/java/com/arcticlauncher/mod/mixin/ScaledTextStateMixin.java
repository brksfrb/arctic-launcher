package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.9
import com.arcticlauncher.mod.ScaledInter;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import net.minecraft.client.gui.Font;
//#if MC >= 26.1
import net.minecraft.client.renderer.state.gui.GuiTextRenderState;
//#else
import net.minecraft.client.gui.render.state.GuiTextRenderState;
//#endif
import org.spongepowered.asm.mixin.Mixin;

/**
 * GUI text is laid out once its pose is known: the pose's scale picks the smooth font's size bucket for the
 * letters (see {@link ScaledInter}). Only while this text is prepared.
 */
@Mixin(GuiTextRenderState.class)
abstract class ScaledTextStateMixin {
	@WrapMethod(method = "ensurePrepared")
	private Font.PreparedText arctic$scaled(Operation<Font.PreparedText> original) {
		ScaledInter.preparing(((GuiTextRenderState) (Object) this).pose);
		try {
			return original.call();
		} finally {
			ScaledInter.prepared();
		}
	}
}
//#endif
