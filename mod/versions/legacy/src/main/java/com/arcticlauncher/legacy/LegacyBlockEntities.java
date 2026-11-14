package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.BlockEntityCulling;
import com.arcticlauncher.client.feature.SignLineCache;
import net.minecraft.block.entity.BannerBlockEntity;
import net.minecraft.block.entity.BlockEntity;
import net.minecraft.block.entity.ChestBlockEntity;
import net.minecraft.block.entity.EnderChestBlockEntity;
import net.minecraft.block.entity.SignBlockEntity;
import net.minecraft.block.entity.SkullBlockEntity;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.render.CameraView;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Box;

/**
 * Chests, signs and the like in the world, cheaper (see {@link BlockEntityCulling}): only the ones in
 * view are drawn, and sign lines are laid out once instead of every frame ({@link SignLineCache}).
 * Render thread only.
 */
public final class LegacyBlockEntities {
	/** The game's own kinds that draw inside their block and one block around it (exactly these classes, no subclasses). */
	private static final Class<?>[] CULLED = {
			ChestBlockEntity.class, EnderChestBlockEntity.class, SignBlockEntity.class, SkullBlockEntity.class, BannerBlockEntity.class,
			//#if MC >= 1.11
			net.minecraft.block.entity.ShulkerBoxBlockEntity.class,
			//#endif
			//#if MC >= 1.12
			net.minecraft.block.entity.BedBlockEntity.class,
			//#endif
	};

	/** Tells the game's frustum about a box. */
	private static final class FrustumView implements BlockEntityCulling.View {
		CameraView frustum;

		@Override
		public boolean sees(double minX, double minY, double minZ, double maxX, double maxY, double maxZ) {
			return frustum.isBoxInFrustum(new Box(minX, minY, minZ, maxX, maxY, maxZ));
		}
	}

	private static final FrustumView VIEW = new FrustumView();
	/** The world's block entities are being drawn (inside renderEntities). */
	private static boolean drawingWorld;
	public static final SignLineCache SIGNS = new SignLineCache();
	/** Each laid-out line's text with formatting codes. */
	public static final SignLineCache SIGN_TEXT = new SignLineCache();
	/** The sign being drawn, for its kept lines. */
	private static SignBlockEntity sign;

	private LegacyBlockEntities() {}

	/** Known to draw only in and around its own block? */
	static boolean known(BlockEntity entity) {
		Class<?> type = entity.getClass();
		for (Class<?> c : CULLED) {
			if (c == type) {
				return true;
			}
		}
		return false;
	}

	/** The world's entities and block entities are about to be drawn with this frustum. */
	public static void beginWorld(CameraView frustum, TextRenderer font) {
		VIEW.frustum = frustum;
		drawingWorld = frustum != null;
		if (font != null) {
			float probe = font.getStringWidth("The quick brown fox WMi. 0123456789 [:]"); // i18n-ignore
			SIGNS.font(font, probe);
			SIGN_TEXT.font(font, probe);
		}
	}

	public static void endWorld() {
		drawingWorld = false;
		VIEW.frustum = null;
	}

	/** Should the world's block entity be left out this frame? Camera at {@code camX, camY, camZ}. */
	public static boolean skip(BlockEntity entity, double camX, double camY, double camZ) {
		if (!drawingWorld || entity == null || !known(entity)) {
			return false;
		}
		BlockPos pos = entity.getPos();
		return BlockEntityCulling.skip(ArcticClient.config(), pos.getX(), pos.getY(), pos.getZ(), camX, camY, camZ, VIEW);
	}

	/** The sign about to be drawn (its lines are looked up while it is). */
	public static void drawingSign(SignBlockEntity entity) {
		sign = entity;
	}

	public static SignBlockEntity sign() {
		return sign;
	}
}
