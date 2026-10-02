package com.arcticlauncher.polonium.mixin;

import com.arcticlauncher.polonium.KeptStates;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.renderer.entity.state.ArmedEntityRenderState;
import net.minecraft.client.renderer.item.ItemModelResolver;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;

/**
 * A kept state's held items (see {@link KeptStates}): their models and
 * copies left as they are while the player holds the same items, on the
 * same tick.
 */
@Mixin(ArmedEntityRenderState.class)
abstract class ArmedStateKeptSlotsMixin implements KeptStates.Slots {
	@Unique
	private final Object[] polonium$sources = new Object[8];
	@Unique
	private final int[] polonium$ticks = new int[8];

	@Override
	public Object[] polonium$sources() {
		return polonium$sources;
	}

	@Override
	public int[] polonium$ticks() {
		return polonium$ticks;
	}

	@Unique
	private int polonium$itemsVersion;

	@Override
	public int polonium$itemsVersion() {
		return polonium$itemsVersion;
	}

	@Override
	public void polonium$itemsChanged() {
		polonium$itemsVersion++;
	}

	@WrapOperation(method = "extractArmedEntityRenderState", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/item/ItemModelResolver;updateForLiving(Lnet/minecraft/client/renderer/item/ItemStackRenderState;Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/item/ItemDisplayContext;Lnet/minecraft/world/entity/LivingEntity;)V"))
	private static void polonium$keptModel(ItemModelResolver resolver, ItemStackRenderState output, ItemStack item, ItemDisplayContext context,
			LivingEntity entity, Operation<Void> update, @Local(argsOnly = true) ArmedEntityRenderState state) {
		int slot = output == state.rightHandItemState ? KeptStates.RIGHT_MODEL : KeptStates.LEFT_MODEL;
		if (!KeptStates.same(state, slot, item, entity.tickCount)) {
			update.call(resolver, output, item, context, entity);
			((KeptStates.Slots) state).polonium$itemsChanged();
		}
	}

	@WrapOperation(method = "extractArmedEntityRenderState",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/world/item/ItemStack;copy()Lnet/minecraft/world/item/ItemStack;", ordinal = 0))
	private static ItemStack polonium$keptLeftCopy(ItemStack held, Operation<ItemStack> copy, @Local(argsOnly = true) LivingEntity entity,
			@Local(argsOnly = true) ArmedEntityRenderState state) {
		return KeptStates.sameCopy(state, KeptStates.LEFT_COPY, held, entity.tickCount) ? state.leftHandItemStack : copy.call(held);
	}

	@WrapOperation(method = "extractArmedEntityRenderState",
			at = @At(value = "INVOKE", target = "Lnet/minecraft/world/item/ItemStack;copy()Lnet/minecraft/world/item/ItemStack;", ordinal = 1))
	private static ItemStack polonium$keptRightCopy(ItemStack held, Operation<ItemStack> copy, @Local(argsOnly = true) LivingEntity entity,
			@Local(argsOnly = true) ArmedEntityRenderState state) {
		return KeptStates.sameCopy(state, KeptStates.RIGHT_COPY, held, entity.tickCount) ? state.rightHandItemStack : copy.call(held);
	}
}
