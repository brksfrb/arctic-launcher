package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.BlockEntityCulling;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.block.entity.BannerBlockEntity;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.ChestBlockEntity;
import net.minecraft.world.level.block.entity.EnderChestBlockEntity;
import net.minecraft.world.level.block.entity.ShulkerBoxBlockEntity;
import net.minecraft.world.level.block.entity.SignBlockEntity;
import net.minecraft.world.level.block.entity.SkullBlockEntity;
import net.minecraft.world.phys.AABB;

/**
 * Chests, signs and the like outside the view are not drawn (see {@link BlockEntityCulling}). The
 * frustum is the game's own: before 1.15 the one the world's entities are drawn with, from 1.15 the
 * one the frame's entities were tested against. Render thread.
 */
public final class BlockEntityCull {
	/** The game's own kinds that draw inside their block and one block around it (exactly these classes, no subclasses). */
	private static final Class<?>[] CULLED = {
			ChestBlockEntity.class, EnderChestBlockEntity.class, SignBlockEntity.class, SkullBlockEntity.class, BannerBlockEntity.class,
			ShulkerBoxBlockEntity.class,
			//#if MC < 26.2
			net.minecraft.world.level.block.entity.BedBlockEntity.class,
			//#endif
			//#if MC >= 1.14
			net.minecraft.world.level.block.entity.TrappedChestBlockEntity.class,
			net.minecraft.world.level.block.entity.BellBlockEntity.class,
			net.minecraft.world.level.block.entity.LecternBlockEntity.class,
			net.minecraft.world.level.block.entity.CampfireBlockEntity.class,
			//#endif
			//#if MC >= 1.20
			net.minecraft.world.level.block.entity.HangingSignBlockEntity.class,
			net.minecraft.world.level.block.entity.DecoratedPotBlockEntity.class,
			//#endif
	};

	static {
		// Shader packs draw block entities a second time from the sun, with a view of their own; portal mods
		// draw the world again through each portal. Culling by the camera's view would leave holes there.
		FabricLoader loader = FabricLoader.getInstance();
		BlockEntityCulling.shadowPasses(loader.isModLoaded("iris") || loader.isModLoaded("oculus") || loader.isModLoaded("optifabric")
				|| loader.isModLoaded("immersive_portals") || loader.isModLoaded("imm_ptl_core"));
	}

	//#if MC >= 1.15
	private static final class FrustumView implements BlockEntityCulling.View {
		net.minecraft.client.renderer.culling.Frustum frustum;

		@Override
		public boolean sees(double minX, double minY, double minZ, double maxX, double maxY, double maxZ) {
			return frustum.isVisible(new AABB(minX, minY, minZ, maxX, maxY, maxZ));
		}
	}
	//#else
	private static final class FrustumView implements BlockEntityCulling.View {
		net.minecraft.client.renderer.culling.Culler frustum;

		@Override
		public boolean sees(double minX, double minY, double minZ, double maxX, double maxY, double maxZ) {
			return frustum.isVisible(new AABB(minX, minY, minZ, maxX, maxY, maxZ));
		}
	}

	/** The world's entities and block entities are being drawn with this frustum (before 1.15). */
	public static void beginWorld(net.minecraft.client.renderer.culling.Culler frustum) {
		VIEW.frustum = frustum;
	}

	public static void endWorld() {
		VIEW.frustum = null;
	}
	//#endif

	private static final FrustumView VIEW = new FrustumView();

	private BlockEntityCull() {}

	/** Known to draw only in and around its own block? */
	private static boolean known(BlockEntity entity) {
		Class<?> type = entity.getClass();
		for (Class<?> c : CULLED) {
			if (c == type) {
				return true;
			}
		}
		return false;
	}

	/** Should this block entity be left out of the world this frame? */
	public static boolean skip(BlockEntity entity) {
		com.arcticlauncher.client.config.ClientConfig config = ArcticClient.config();
		if (entity == null || !BlockEntityCulling.active(config) || !known(entity)) {
			return false;
		}
		//#if MC >= 1.15
		Object frustum = BlockEntityCulling.frustum();
		if (!(frustum instanceof net.minecraft.client.renderer.culling.Frustum)) {
			// Not inside the world's drawing (a preview, a menu), or the frame's view isn't known: as the game does.
			return false;
		}
		VIEW.frustum = (net.minecraft.client.renderer.culling.Frustum) frustum;
		//#else
		if (VIEW.frustum == null) {
			return false;
		}
		//#endif
		BlockPos pos = entity.getBlockPos();
		double camX = 0;
		double camY = 0;
		double camZ = 0;
		if (BlockEntityCulling.distance(config) < BlockEntityCulling.GAME_DISTANCE) {
			//#if MC >= 1.15
			//#if MC >= 26.2
			net.minecraft.world.phys.Vec3 cam = net.minecraft.client.Minecraft.getInstance().gameRenderer.mainCamera().position();
			//#elif MC >= 1.21.11
			net.minecraft.world.phys.Vec3 cam = net.minecraft.client.Minecraft.getInstance().gameRenderer.getMainCamera().position();
			//#else
			net.minecraft.world.phys.Vec3 cam = net.minecraft.client.Minecraft.getInstance().gameRenderer.getMainCamera().getPosition();
			//#endif
			camX = cam.x;
			camY = cam.y;
			camZ = cam.z;
			//#else
			camX = net.minecraft.client.renderer.blockentity.BlockEntityRenderDispatcher.xOff;
			camY = net.minecraft.client.renderer.blockentity.BlockEntityRenderDispatcher.yOff;
			camZ = net.minecraft.client.renderer.blockentity.BlockEntityRenderDispatcher.zOff;
			//#endif
		}
		return BlockEntityCulling.skip(config, pos.getX(), pos.getY(), pos.getZ(), camX, camY, camZ, VIEW);
	}
}
