package com.arcticlauncher.mod.mixin;

//#if MC >= 1.13
import com.arcticlauncher.mod.BadgeTexture;
import com.arcticlauncher.mod.TextDrawScale;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Unique;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
//#if MC >= 1.21.4
import org.spongepowered.asm.mixin.injection.Redirect;
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
 * is therefore on the path. A {@code renderChar} invoker redirect is also applied so a future
 * inlined renderChar cannot bypass snap. 1.13–1.14 use the older {@code render(TextureManager, …)}
 * signature and the same x/y modification.
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
	@Redirect(method = "renderChar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/font/glyphs/BakedSheetGlyph$GlyphInstance;x()F"))
	//#else
	@Redirect(method = "renderChar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/font/glyphs/BakedGlyph$GlyphInstance;x()F"))
	//#endif
	private float arctic$charX(BadgeGlyphInstanceAccess inst) {
		return arctic$snapInstance(inst)[0];
	}

	//#if MC >= 1.21.9
	@Redirect(method = "renderChar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/font/glyphs/BakedSheetGlyph$GlyphInstance;y()F"))
	//#else
	@Redirect(method = "renderChar", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/client/gui/font/glyphs/BakedGlyph$GlyphInstance;y()F"))
	//#endif
	private float arctic$charY(BadgeGlyphInstanceAccess inst) {
		return arctic$snapInstance(inst)[1];
	}

	@Unique
	private float[] arctic$snapInstance(BadgeGlyphInstanceAccess inst) {
		arctic$snap[0] = inst.arctic$x();
		arctic$snap[1] = inst.arctic$y();
		if (BadgeTexture.isBadge(this)) {
			TextDrawScale.snapOrigin(inst.arctic$x(), inst.arctic$y(), arctic$snap);
		}
		return arctic$snap;
	}
	//#endif
}
//#endif
