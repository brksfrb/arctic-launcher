package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import net.minecraft.client.player.AbstractClientPlayer;
//#if MC >= 1.20.5
import net.minecraft.core.Holder;
//#elif MC >= 1.16
import net.minecraft.world.entity.ai.attributes.Attribute;
//#else
import net.minecraft.world.entity.ai.attributes.AttributeInstance;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Constant;
import org.spongepowered.asm.mixin.injection.ModifyConstant;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * Dynamic FOV: the view's change with speed and with a drawn bow, each switched off on its own. The game's number
 * is flying (x1.1), the movement speed against the walking speed, and the bow (x1 - 0.15 at most): the speed is
 * read as the walking speed (no change) and the constants are neutral.
 */
@Mixin(AbstractClientPlayer.class)
abstract class DynamicFovMixin {
	//#if MC >= 1.20.5
	@Redirect(method = "getFieldOfViewModifier", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/player/AbstractClientPlayer;getAttributeValue(Lnet/minecraft/core/Holder;)D"), require = 0)
	private double arctic$speed(AbstractClientPlayer self, Holder<?> attribute) {
		ClientConfig c = ArcticClient.config();
		return c != null && c.fovNoSpeed ? com.arcticlauncher.mod.Compat.walkingSpeed(self) : self.getAttributeValue((Holder) attribute);
	}
	//#elif MC >= 1.16
	@Redirect(method = "getFieldOfViewModifier", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/player/AbstractClientPlayer;getAttributeValue(Lnet/minecraft/world/entity/ai/attributes/Attribute;)D"), require = 0)
	private double arctic$speed(AbstractClientPlayer self, Attribute attribute) {
		ClientConfig c = ArcticClient.config();
		return c != null && c.fovNoSpeed ? com.arcticlauncher.mod.Compat.walkingSpeed(self) : self.getAttributeValue(attribute);
	}
	//#else
	@Redirect(method = "getFieldOfViewModifier", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/world/entity/ai/attributes/AttributeInstance;getValue()D"), require = 0)
	private double arctic$speed(AttributeInstance attribute) {
		ClientConfig c = ArcticClient.config();
		return c != null && c.fovNoSpeed ? com.arcticlauncher.mod.Compat.walkingSpeed((AbstractClientPlayer) (Object) this) : attribute.getValue();
	}
	//#endif

	@ModifyConstant(method = "getFieldOfViewModifier", constant = @Constant(floatValue = 1.1F), require = 0)
	private float arctic$flying(float value) {
		ClientConfig c = ArcticClient.config();
		return c != null && c.fovNoSpeed ? 1F : value;
	}

	@ModifyConstant(method = "getFieldOfViewModifier", constant = @Constant(floatValue = 0.15F), require = 0)
	private float arctic$bow(float value) {
		ClientConfig c = ArcticClient.config();
		return c != null && c.fovNoBow ? 0F : value;
	}
}
//#endif
