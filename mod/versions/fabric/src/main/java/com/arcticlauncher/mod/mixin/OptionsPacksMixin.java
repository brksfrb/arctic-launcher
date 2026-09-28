package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.ArcticPacks;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.Options;
import net.minecraft.server.packs.repository.PackRepository;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * At startup the smooth font pack follows Fancy (which the launcher may
 * have just switched), before the first resource load, so no extra reload.
 */
@Mixin(Options.class)
abstract class OptionsPacksMixin {
	@Inject(method = "loadSelectedResourcePacks", at = @At("HEAD"))
	private void arctic$followFancy(PackRepository repo, CallbackInfo ci) {
		if (ArcticClient.config() == null) {
			return;
		}
		Options options = (Options) (Object) this;
		boolean on = ArcticClient.config().fancy;
		if (options.resourcePacks.contains(ArcticPacks.SMOOTH_FONT) == on) {
			return;
		}
		List<String> packs = new ArrayList<>(options.resourcePacks);
		if (on) {
			packs.add(ArcticPacks.SMOOTH_FONT);
		} else {
			packs.remove(ArcticPacks.SMOOTH_FONT);
		}
		options.resourcePacks = packs;
	}
}
//#endif
