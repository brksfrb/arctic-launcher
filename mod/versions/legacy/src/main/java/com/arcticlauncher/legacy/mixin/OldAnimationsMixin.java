//#if MC < 1.9
package com.arcticlauncher.legacy.mixin;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.mojang.blaze3d.platform.GlStateManager;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.item.HeldItemRenderer;
import net.minecraft.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.Redirect;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * 1.7 animations in the first-person hand, 1.8.9. The game draws an item that is being used (a
 * blocking sword, food, a drawn bow) without the swing; 1.7 swung it all the same. The bow also
 * stretched along its own length instead of straight ahead. The hand code is otherwise the same
 * in both versions (checked against 1.7.10).
 */
@Mixin(HeldItemRenderer.class)
abstract class OldAnimationsMixin {
	private static final String OFFSET = "Lnet/minecraft/client/render/item/HeldItemRenderer;applyEquipAndSwingOffset(FF)V";
	private static final String BOW = "Lnet/minecraft/client/render/item/HeldItemRenderer;applyBowTransformation"
			+ "(FLnet/minecraft/client/network/AbstractClientPlayerEntity;)V";

	/** The frame's partial tick, for the swing progress. */
	@Unique
	private float arctic$partial;

	@Shadow
	private ItemStack mainHand;

	@Shadow
	private void applyEquipAndSwingOffset(float equip, float swing) {}

	@Shadow
	private void applyBowTransformation(float partial, AbstractClientPlayerEntity player) {}

	@Inject(method = "renderArmHoldingItem", at = @At("HEAD"))
	private void arctic$partialTick(float partial, CallbackInfo ci) {
		arctic$partial = partial;
	}

	/** The equip offset with the swing in it, for the use actions that the game leaves it out of. */
	@Unique
	private void arctic$offset(float equip, float swing, boolean on) {
		MinecraftClient client = MinecraftClient.getInstance();
		if (on && client.player != null) {
			applyEquipAndSwingOffset(equip, client.player.getHandSwingProgress(arctic$partial));
		} else {
			applyEquipAndSwingOffset(equip, swing);
		}
	}

	// The five calls in renderArmHoldingItem, in order: no action, eat and drink, block, bow, not using anything.
	@Redirect(method = "renderArmHoldingItem", at = @At(value = "INVOKE", target = OFFSET, ordinal = 0), require = 0)
	private void arctic$useNone(HeldItemRenderer self, float equip, float swing) {
		arctic$offset(equip, swing, ArcticClient.config().oldUseSwing);
	}

	@Redirect(method = "renderArmHoldingItem", at = @At(value = "INVOKE", target = OFFSET, ordinal = 1), require = 0)
	private void arctic$useEat(HeldItemRenderer self, float equip, float swing) {
		arctic$offset(equip, swing, ArcticClient.config().oldUseSwing);
	}

	@Redirect(method = "renderArmHoldingItem", at = @At(value = "INVOKE", target = OFFSET, ordinal = 2), require = 0)
	private void arctic$useBlock(HeldItemRenderer self, float equip, float swing) {
		arctic$offset(equip, swing, ArcticClient.config().oldBlockHit);
	}

	@Redirect(method = "renderArmHoldingItem", at = @At(value = "INVOKE", target = OFFSET, ordinal = 3), require = 0)
	private void arctic$useBow(HeldItemRenderer self, float equip, float swing) {
		arctic$offset(equip, swing, ArcticClient.config().oldUseSwing);
	}

	/** The bow: the game's, then its stretch (straight ahead) swapped for 1.7's (along the bow's own length). */
	@Redirect(method = "renderArmHoldingItem", at = @At(value = "INVOKE", target = BOW), require = 0)
	private void arctic$bow(HeldItemRenderer self, float partial, AbstractClientPlayerEntity player) {
		applyBowTransformation(partial, player);
		ClientConfig config = ArcticClient.config();
		if (!config.oldBow || mainHand == null) {
			return;
		}
		float drawn = (mainHand.getMaxUseTime() - (player.getItemUseTicks() - partial + 1f)) / 20f;
		drawn = (drawn * drawn + drawn * 2f) / 3f;
		if (drawn > 1f) {
			drawn = 1f;
		}
		float stretch = 1f + drawn * 0.2f;
		GlStateManager.scale(1f, 1f, 1f / stretch);
		GlStateManager.rotate(-335f, 0f, 0f, 1f);
		GlStateManager.rotate(-50f, 0f, 1f, 0f);
		GlStateManager.translate(0f, 0.5f, 0f);
		GlStateManager.scale(1f, 1f, stretch);
		GlStateManager.translate(0f, -0.5f, 0f);
		GlStateManager.rotate(50f, 0f, 1f, 0f);
		GlStateManager.rotate(335f, 0f, 0f, 1f);
	}
}
//#endif
