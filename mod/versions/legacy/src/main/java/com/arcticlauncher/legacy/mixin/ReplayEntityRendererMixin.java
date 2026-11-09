package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.replay.Replays;
import java.util.HashSet;
import java.util.Set;
import net.minecraft.client.render.entity.EntityRenderDispatcher;
import net.minecraft.client.render.CameraView;
import net.minecraft.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** A missing renderer must only suppress that entity, and only in a replay. */
@Mixin(EntityRenderDispatcher.class)
abstract class ReplayEntityRendererMixin {
	@Unique
	private final Set<Class<?>> arctic$missingReplayRenderers = new HashSet<Class<?>>();

	@Inject(method = "shouldRender", at = @At("HEAD"), cancellable = true)
	private void arctic$skipMissingReplayRenderer(Entity entity, CameraView frustum, double x, double y, double z,
			CallbackInfoReturnable<Boolean> cir) {
		if (!Replays.watching()) {
			arctic$missingReplayRenderers.clear();
			return;
		}
		if (((EntityRenderDispatcher) (Object) this).getRenderer(entity) == null) {
			if (arctic$missingReplayRenderers.add(entity.getClass())) {
				com.arcticlauncher.legacy.ArcticLegacy.LOG.warn("replay: skipping entity without renderer: {} ({}); check resources and recording mods/registries",
						entity, entity.getClass().getName());
			}
			cir.setReturnValue(false);
		}
	}
}
