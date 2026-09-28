package com.arcticlauncher.mod.mixin;

//#if MC >= 26.2
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.state.level.BlockOutlineRenderState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArgs;
import org.spongepowered.asm.mixin.injection.invoke.arg.Args;

/** Your block outline color and thickness (high-contrast mode is left as it is). */
@Mixin(LevelRenderer.class)
abstract class BlockOutlineMixin {
	private static final int STATE = 3;
	private static final int COLOR = 4;
	private static final int WIDTH = 5;

	@ModifyArgs(method = "submitBlockOutline", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/LevelRenderer;submitHitOutline(Lcom/mojang/blaze3d/vertex/PoseStack;"
					+ "Lnet/minecraft/client/renderer/SubmitNodeCollector;Lnet/minecraft/client/renderer/rendertype/RenderType;"
					+ "Lnet/minecraft/client/renderer/state/level/BlockOutlineRenderState;IFZ)V"))
	private void arctic$outline(Args args) {
		ClientConfig c = ArcticClient.config();
		BlockOutlineRenderState state = args.get(STATE);
		if (state.highContrast()) {
			return;
		}
		if (c.outlineColor != 0) {
			args.set(COLOR, c.outlineColor);
		}
		if (c.outlineWidth != 1f) {
			float width = args.get(WIDTH);
			args.set(WIDTH, width * c.outlineWidth);
		}
	}
}
//#endif
