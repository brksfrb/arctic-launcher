package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.renderer.LevelRenderer;
//#if MC >= 26.2
import net.minecraft.client.renderer.state.level.BlockOutlineRenderState;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import org.spongepowered.asm.mixin.injection.ModifyArgs;
import org.spongepowered.asm.mixin.injection.invoke.arg.Args;

/** Your block outline color and thickness (high-contrast mode is left as it is). */
@Mixin(LevelRenderer.class)
abstract class BlockOutlineMixin {
	//#if MC >= 26.2
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
	//#elif MC >= 1.21.11
	// 1.21.11 - 26.1: the outline shape is drawn with a packed colour and its own line width.
	@ModifyArgs(method = "renderHitOutline", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/ShapeRenderer;renderShape(Lcom/mojang/blaze3d/vertex/PoseStack;Lcom/mojang/blaze3d/vertex/VertexConsumer;Lnet/minecraft/world/phys/shapes/VoxelShape;DDDIF)V"))
	private void arctic$outline(Args args) {
		ClientConfig c = ArcticClient.config();
		if (c.outlineColor != 0) {
			args.set(6, c.outlineColor);
		}
		if (c.outlineWidth != 1f) {
			float width = args.get(7);
			args.set(7, width * c.outlineWidth);
		}
	}
	//#elif MC >= 1.21.2
	// 1.21.2 - 1.21.10: a packed colour; the line width is shared by every line drawn (not changed).
	@ModifyArg(method = "renderHitOutline", index = 6, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/ShapeRenderer;renderShape(Lcom/mojang/blaze3d/vertex/PoseStack;Lcom/mojang/blaze3d/vertex/VertexConsumer;Lnet/minecraft/world/phys/shapes/VoxelShape;DDDI)V"))
	private int arctic$outline(int color) {
		int mine = ArcticClient.config().outlineColor;
		return mine != 0 ? mine : color;
	}
	//#else
	// Before 1.21.2: the colour as four floats (the game's is black at 40%).
	@ModifyArgs(method = "renderHitOutline", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/LevelRenderer;renderShape(Lcom/mojang/blaze3d/vertex/PoseStack;Lcom/mojang/blaze3d/vertex/VertexConsumer;Lnet/minecraft/world/phys/shapes/VoxelShape;DDDFFFF)V"))
	private void arctic$outline(Args args) {
		int c = ArcticClient.config().outlineColor;
		if (c == 0) {
			return;
		}
		args.set(6, ((c >> 16) & 0xFF) / 255f);
		args.set(7, ((c >> 8) & 0xFF) / 255f);
		args.set(8, (c & 0xFF) / 255f);
		args.set(9, ((c >>> 24) & 0xFF) / 255f);
	}
	//#endif
}
