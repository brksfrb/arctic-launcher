package com.arcticlauncher.mod.mixin;

//#if MC >= 1.15
import com.arcticlauncher.mod.BadgeTexture;
import com.arcticlauncher.mod.TextDrawScale;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
//#if MC >= 1.21.9
import net.minecraft.client.gui.font.glyphs.BakedSheetGlyph;
//#else
import net.minecraft.client.gui.font.glyphs.BakedGlyph;
//#endif

/**
 * Tab-list (and billboarded nametag) badge quads snap their origin to a whole framebuffer pixel so
 * a 1:1 blit does not land on a half-pixel and soften under linear filtering. Rotated tags are left
 * alone. Uses the clip matrix captured when the text's projected size was measured.
 */
//#if MC >= 1.21.9
@Mixin(BakedSheetGlyph.class)
//#else
@Mixin(BakedGlyph.class)
//#endif
abstract class BadgeSnapMixin {
	@Unique private static final float[] arctic$snap = new float[2];
	@Unique private static float arctic$snappedY;
	@Unique private static boolean arctic$didSnap;

	@ModifyVariable(method = "render", at = @At("HEAD"), argsOnly = true, ordinal = 0)
	private float arctic$snapX(float x, boolean italic, float x0, float y) {
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

}
//#endif
