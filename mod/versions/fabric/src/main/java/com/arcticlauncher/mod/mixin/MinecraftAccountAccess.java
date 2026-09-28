package com.arcticlauncher.mod.mixin;

//#if MC >= 26.1
import com.mojang.authlib.minecraft.UserApiService;
//#if MC >= 26.3
import com.mojang.authlib.services.ProfileResult;
//#else
import com.mojang.authlib.yggdrasil.ProfileResult;
//#endif
import java.net.Proxy;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.Minecraft;
import net.minecraft.client.User;
import net.minecraft.client.multiplayer.ProfileKeyPairManager;
import net.minecraft.client.multiplayer.chat.report.ReportingContext;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The per-account parts of Minecraft, for switching accounts in game. */
@Mixin(Minecraft.class)
public interface MinecraftAccountAccess {
	@Mutable
	@Accessor("user")
	void arctic$setUser(User user);

	@Mutable
	@Accessor("userApiService")
	void arctic$setUserApiService(UserApiService service);

	@Mutable
	@Accessor("userPropertiesFuture")
	void arctic$setUserPropertiesFuture(CompletableFuture<UserApiService.UserProperties> future);

	@Mutable
	@Accessor("profileFuture")
	void arctic$setProfileFuture(CompletableFuture<ProfileResult> future);

	@Mutable
	@Accessor("profileKeyPairManager")
	void arctic$setProfileKeyPairManager(ProfileKeyPairManager manager);

	@Accessor("reportingContext")
	void arctic$setReportingContext(ReportingContext context);

	@Accessor("proxy")
	Proxy arctic$proxy();
}
//#endif
