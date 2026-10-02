//#if MC >= 26.1 && MC < 26.2
package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.ParallelExtract;
import com.arcticlauncher.polonium.Workers;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import net.minecraft.core.BlockPos;
import net.minecraft.util.Mth;
import net.minecraft.world.TickRateManager;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.phys.Vec3;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The game's {@code extractVisibleEntities} (26.1: still in the level
 * renderer), with the per-entity work spread over threads as on 26.2 (see
 * LevelExtractorMixin): which entities are visible, then their render
 * states, worked out on several threads for renderers known to be safe;
 * everything with side effects stays on this thread, and states are added in
 * the game's order. With few entities, or when Polonium has stepped aside,
 * the game's own code runs.
 */
@Mixin(value = LevelRenderer.class, priority = 1500)
abstract class LevelRendererExtractMixin {
	@Shadow
	@Final
	private Minecraft minecraft;

	@Shadow
	@Final
	private EntityRenderDispatcher entityRenderDispatcher;

	@Shadow
	private ClientLevel level;

	@Shadow
	protected abstract boolean shouldShowEntityOutlines();

	@Shadow
	public abstract boolean isSectionCompiledAndVisible(BlockPos pos);

	@Shadow
	protected abstract EntityRenderState extractEntity(Entity entity, float partialTickTime);

	@Inject(method = "extractVisibleEntities", at = @At("HEAD"), cancellable = true)
	private void polonium$parallel(Camera camera, Frustum frustum, DeltaTracker deltaTracker, LevelRenderState output, CallbackInfo ci) {
		if (!ParallelExtract.enabled()) {
			return;
		}
		Vec3 cameraPos = camera.position();
		double camX = cameraPos.x();
		double camY = cameraPos.y();
		double camZ = cameraPos.z();
		TickRateManager tickRateManager = this.minecraft.level.tickRateManager();
		boolean shouldShowEntityOutlines = this.shouldShowEntityOutlines();
		Entity.setViewScale(
				Mth.clamp(this.minecraft.options.getEffectiveRenderDistance() / 8.0, 1.0, 2.5) * this.minecraft.options.entityDistanceScaling().get());
		List<Entity> all = new ArrayList<>();
		for (Entity entity : this.level.entitiesForRendering()) {
			all.add(entity);
		}
		Workers.load(all.size());
		boolean[] trustedAll = new boolean[all.size()];
		LocalPlayer player = this.minecraft.player;
		boolean[] seen = ParallelExtract.visible(all, this.entityRenderDispatcher, e -> {
			if (!this.entityRenderDispatcher.shouldRender(e, frustum, camX, camY, camZ) && !e.hasIndirectPassenger(player)) {
				return false;
			}
			BlockPos pos = e.blockPosition();
			return this.level.isOutsideBuildHeight(pos.getY()) || this.isSectionCompiledAndVisible(pos);
		}, trustedAll);
		List<Entity> visible = new ArrayList<>(all.size());
		float[] partials = new float[all.size()];
		boolean[] trusted = new boolean[all.size()];
		// The same for every entity but those the tick rate freezes.
		float running = deltaTracker.getGameTimeDeltaPartialTick(true);
		float frozen = deltaTracker.getGameTimeDeltaPartialTick(false);
		for (int i = 0; i < all.size(); i++) {
			Entity entity = all.get(i);
			if (seen[i]
					&& (entity != camera.entity() || camera.isDetached()
							|| camera.entity() instanceof LivingEntity && ((LivingEntity) camera.entity()).isSleeping())
					&& (!(entity instanceof LocalPlayer) || camera.entity() == entity)) {
				if (entity.tickCount == 0) {
					entity.xOld = entity.getX();
					entity.yOld = entity.getY();
					entity.zOld = entity.getZ();
				}
				trusted[visible.size()] = trustedAll[i];
				partials[visible.size()] = tickRateManager.isEntityFrozen(entity) ? frozen : running;
				visible.add(entity);
			}
		}
		int before = output.entityRenderStates.size();
		ParallelExtract.extract(visible, partials, trusted, this::extractEntity, output.entityRenderStates);
		// The glow outlines need to know now (on 26.2 that's worked out later).
		ParallelExtract.finish();
		if (shouldShowEntityOutlines) {
			for (int i = before; i < output.entityRenderStates.size(); i++) {
				if (output.entityRenderStates.get(i).appearsGlowing()) {
					output.haveGlowingEntities = true;
					break;
				}
			}
		}
		output.lastEntityRenderStateCount = output.entityRenderStates.size();
		ci.cancel();
	}
}
//#endif
