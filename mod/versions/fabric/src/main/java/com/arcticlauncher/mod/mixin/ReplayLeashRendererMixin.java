package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.client.replay.Replays;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import net.minecraft.client.Minecraft;
//#if MC >= 1.21
import net.minecraft.client.renderer.entity.EntityRenderer;
//#else
import net.minecraft.client.renderer.entity.MobRenderer;
//#endif
import net.minecraft.world.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** Hide only the leash when its holder cannot supply lighting or a culling box in replay. */
//#if MC >= 1.21
@Mixin(EntityRenderer.class)
//#else
@Mixin(MobRenderer.class)
//#endif
abstract class ReplayLeashRendererMixin {
	//#if MC >= 1.21.2
	@ModifyExpressionValue(method = {"shouldRender", "extract" + "RenderState"}, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/Leashable;getLeashHolder()Lnet/minecraft/world/entity/Entity;"))
	//#elif MC >= 1.21
	@ModifyExpressionValue(method = {"shouldRender", "render"}, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/Leashable;getLeashHolder()Lnet/minecraft/world/entity/Entity;"))
	//#elif MC >= 1.15
	@ModifyExpressionValue(method = {"shouldRender", "render"}, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/Mob;getLeashHolder()Lnet/minecraft/world/entity/Entity;"))
	//#else
	@ModifyExpressionValue(method = {"shouldRender", "renderLeash"}, at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/Mob;getLeashHolder()Lnet/minecraft/world/entity/Entity;"))
	//#endif
	private Entity arctic$replayLeashHolder(Entity holder) {
		if (holder != null && Replays.watching()
				&& Minecraft.getInstance().getEntityRenderDispatcher().getRenderer(holder) == null) {
			return null;
		}
		return holder;
	}
}
