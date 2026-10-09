package com.arcticlauncher.mod.mixin;

//#if MC >= 1.18
import com.mojang.authlib.minecraft.UserApiService;
//#endif
//#if MC >= 26.3
import com.mojang.authlib.services.ProfileResult;
//#elif MC >= 1.20.2
import com.mojang.authlib.yggdrasil.ProfileResult;
//#endif
import java.net.Proxy;
//#if MC >= 1.20.2
import java.util.concurrent.CompletableFuture;
//#endif
import net.minecraft.client.Minecraft;
import net.minecraft.client.User;
//#if MC >= 1.19
import net.minecraft.client.multiplayer.ProfileKeyPairManager;
import net.minecraft.client.multiplayer.chat.report.ReportingContext;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * The per-account parts of Minecraft, for switching accounts in game. Which parts there are
 * depends on the version: the user on every one, its API service from 1.18, chat signing keys and
 * reporting from 1.19, the profile from 1.20.2 and the user's properties from 1.20.3.
 */
@Mixin(Minecraft.class)
public interface MinecraftAccountAccess {
	@Mutable
	@Accessor("user")
	void arctic$setUser(User user);

	@Accessor("proxy")
	Proxy arctic$proxy();

	//#if MC >= 1.18
	@Mutable
	@Accessor("userApiService")
	void arctic$setUserApiService(UserApiService service);
	//#endif

	//#if MC >= 1.20.3
	@Mutable
	@Accessor("userPropertiesFuture")
	void arctic$setUserPropertiesFuture(CompletableFuture<UserApiService.UserProperties> future);
	//#endif

	//#if MC >= 1.20.2
	@Mutable
	@Accessor("profileFuture")
	void arctic$setProfileFuture(CompletableFuture<ProfileResult> future);
	//#endif

	//#if MC >= 1.19
	@Mutable
	@Accessor("profileKeyPairManager")
	void arctic$setProfileKeyPairManager(ProfileKeyPairManager manager);

	@Mutable
	@Accessor("reportingContext")
	void arctic$setReportingContext(ReportingContext context);
	//#endif
}
