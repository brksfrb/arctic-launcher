package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13
import com.arcticlauncher.mod.BadgeTexture;
import com.arcticlauncher.mod.TextDrawScale;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
//#if MC >= 1.21.4
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import org.spongepowered.asm.mixin.injection.Coerce;
//#endif
//#if MC >= 1.21.9
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
//#else
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
//#endif

/**
 * Tab-list (and billboarded nametag) badge quads snap their origin to a whole framebuffer pixel so
 * a 1:1 blit does not land on a half-pixel and soften under linear filtering. Rotated tags are left
 * alone. Uses the clip matrix captured when the text's projected size was measured.
 *
 * <p>On 1.21.4+, the public entry is {@code renderChar}, which reads the glyph instance's x/y and
 * then calls the (private) {@code render} overload with those values. The hook on {@code render}
 * is therefore on the path. A chain-friendly {@code @WrapOperation} on {@code renderChar}'s x()/y()
 * calls covers a future inlined renderChar. 1.13–1.14 use the older {@code render(TextureManager, …)}
 * signature and the same x/y modification.
 */
//#if MC >= 1.21.9
@Mixin(BakedSheetGlyph.class)
//#else
@Mixin(BakedGlyph.class)
//#endif
abstract class BadgeSnapMixin {
	/** Render-thread only: text is prepared and drawn on the client thread. */
	@Unique private static final float[] arctic$snap = new float[2];
	@Unique private static float arctic$snappedY;
	@Unique private static boolean arctic$didSnap;

	@ModifyVariable(method = "render", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	//#if MC >= 1.15
	private float arctic$snapX(float x, boolean italic, float x0, float y) {
	//#else
	private float arctic$snapX(float x, net.minecraft.client.renderer.texture.TextureManager textures,
			boolean italic, float x0, float y) {
	//#endif
		arctic$didSnap = false;
		if (!BadgeTexture.isBadge(this)) {
			return x;
		}
		if (!TextDrawScale.snapOrigin(x, y, arctic$snap)) {
			return x;
		}
		arctic$didSnap = true;
		arctic$snappedY = arctic$snap[1];
		return arctic$snap[0];
	}

	@ModifyVariable(method = "render", at = @At("HEAD"), argsOnly = true, ordinal = 1)
	private float arctic$snapY(float y) {
		return arctic$didSnap ? arctic$snappedY : y;
	}

	//#if MC >= 1.21.4
	//#if MC >= 1.21.9
	@WrapOperation(method = "renderChar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/font/glyphs/BakedSheetGlyph$GlyphInstance;x()F"))
	//#else
	@WrapOperation(method = "renderChar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/font/glyphs/BakedGlyph$GlyphInstance;x()F"))
	//#endif
	private float arctic$charX(@Coerce Object inst, Operation<Float> original) {
		float x = original.call(inst);
		if (!BadgeTexture.isBadge(this)) {
			return x;
		}
		float y = ((BadgeGlyphInstanceAccess) inst).arctic$y();
		return TextDrawScale.snapOrigin(x, y, arctic$snap) ? arctic$snap[0] : x;
	}

	//#if MC >= 1.21.9
	@WrapOperation(method = "renderChar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/font/glyphs/BakedSheetGlyph$GlyphInstance;y()F"))
	//#else
	@WrapOperation(method = "renderChar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/font/glyphs/BakedGlyph$GlyphInstance;y()F"))
	//#endif
	private float arctic$charY(@Coerce Object inst, Operation<Float> original) {
		float y = original.call(inst);
		if (!BadgeTexture.isBadge(this)) {
			return y;
		}
		float x = ((BadgeGlyphInstanceAccess) inst).arctic$x();
		return TextDrawScale.snapOrigin(x, y, arctic$snap) ? arctic$snap[1] : y;
	}
	//#endif
}
//#endif
