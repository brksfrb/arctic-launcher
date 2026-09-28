package com.arcticlauncher.mod.cosmetic;

//#if MC >= 26.1
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.looks.Cosmetics;
import com.arcticlauncher.client.looks.Look;
import com.mojang.blaze3d.vertex.PoseStack;
import java.util.UUID;
import net.minecraft.client.Minecraft;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.model.player.PlayerModel;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.RenderLayerParent;
import net.minecraft.client.renderer.entity.layers.RenderLayer;
import net.minecraft.client.renderer.entity.state.AvatarRenderState;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.client.renderer.texture.OverlayTexture;
import net.minecraft.world.entity.Entity;

/** Draws the Arctic cosmetics a player wears, attached to their model's parts. */
public final class CosmeticsLayer extends RenderLayer<AvatarRenderState, PlayerModel> {
	private static final int WHITE = -1;

	public CosmeticsLayer(RenderLayerParent<AvatarRenderState, PlayerModel> parent) {
		super(parent);
	}

	/** The player a render state is for (by entity id), or null. */
	public static UUID player(AvatarRenderState state) {
		Minecraft mc = Minecraft.getInstance();
		Entity entity = mc.level == null ? null : mc.level.getEntity(state.id);
		return entity == null ? null : entity.getUUID();
	}

	@Override
	public void submit(PoseStack pose, SubmitNodeCollector collector, int light, AvatarRenderState state, float yRot, float xRot) {
		if (state.isInvisible || ArcticClient.looks() == null) {
			return;
		}
		UUID id = player(state);
		Look look = id == null ? null : ArcticClient.looks().lookFor(id);
		if (look == null || look.cosmetics.isEmpty()) {
			return;
		}
		Cosmetics cosmetics = ArcticClient.looks().cosmetics();
		for (String worn : look.cosmetics) {
			if (cosmetics.drawable(worn) == null) {
				continue;
			}
			CosmeticModels.Baked baked = CosmeticModels.get(worn);
			if (baked == null) {
				continue;
			}
			for (CosmeticModels.Piece piece : baked.pieces) {
				pose.pushPose();
				part(piece.attach).translateAndRotate(pose);
				//#if MC >= 26.3
				collector.submitModel(piece, state, pose, RenderTypes.entityTranslucent(baked.texture), light, OverlayTexture.NO_OVERLAY,
						WHITE, null, state.outlineColor);
				//#else
				collector.submitModel(piece, state, pose, RenderTypes.entityTranslucent(baked.texture), light, OverlayTexture.NO_OVERLAY,
						WHITE, null, state.outlineColor, null);
				//#endif
				pose.popPose();
			}
		}
	}

	private ModelPart part(CosmeticModels.Attach attach) {
		PlayerModel model = getParentModel();
		switch (attach) {
			case HEAD:
				return model.head;
			case RIGHT_ARM:
				return model.rightArm;
			case LEFT_ARM:
				return model.leftArm;
			case RIGHT_LEG:
				return model.rightLeg;
			case LEFT_LEG:
				return model.leftLeg;
			default:
				return model.body;
		}
	}
}
//#endif
