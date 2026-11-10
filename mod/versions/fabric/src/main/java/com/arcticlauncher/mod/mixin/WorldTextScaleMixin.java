package com.arcticlauncher.mod.mixin;

//#if MC >= 26.2
import com.arcticlauncher.mod.TextDrawScale;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
import java.util.List;
import net.minecraft.client.renderer.feature.FeatureFrameContext;
import net.minecraft.client.renderer.feature.TextFeatureRenderer;
import org.joml.Matrix4fc;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** Every world text feature: names, signs, holograms, text displays and outlined/glowing labels. */
@Mixin(TextFeatureRenderer.class)
abstract class WorldTextScaleMixin {
	@ModifyExpressionValue(method = "buildGroup", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/renderer/feature/TextFeatureRenderer$Submit;pose()Lorg/joml/Matrix4fc;"))
	private Matrix4fc arctic$pose(Matrix4fc pose) {
		if (TextDrawScale.on()) {
			TextDrawScale.set(TextDrawScale.pixels(pose, 0, 0));
		}
		return pose;
	}

	@WrapMethod(method = "buildGroup")
	private void arctic$group(FeatureFrameContext context, List<TextFeatureRenderer.Submit> submits, Operation<Void> original) {
		float previous = TextDrawScale.current();
		try {
			original.call(context, submits);
		} finally {
			TextDrawScale.restore(previous);
		}
	}
}
//#endif
