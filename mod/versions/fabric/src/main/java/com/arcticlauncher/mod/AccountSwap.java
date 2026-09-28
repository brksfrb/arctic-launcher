package com.arcticlauncher.mod;

import java.util.UUID;
//#if MC >= 26.1
import com.arcticlauncher.mod.mixin.MinecraftAccountAccess;
import com.mojang.authlib.minecraft.UserApiService;
//#if MC >= 26.3
import com.mojang.authlib.services.MinecraftServicesDiscoveryService;
//#else
import com.mojang.authlib.yggdrasil.YggdrasilAuthenticationService;
//#endif
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.Minecraft;
import net.minecraft.client.User;
import net.minecraft.client.multiplayer.ProfileKeyPairManager;
import net.minecraft.client.multiplayer.chat.report.ReportEnvironment;
import net.minecraft.client.multiplayer.chat.report.ReportingContext;
import net.minecraft.util.Util;
//#endif

/**
 * Plays as another account without restarting: the same per-account parts
 * Minecraft sets up at start (the user, its API service and properties,
 * profile, chat signing keys, reporting) are made again for the new one.
 */
public final class AccountSwap {
	private AccountSwap() {}

	/** Returns why it couldn't, or null. Main thread, outside a world. */
	public static String swap(String name, UUID uuid, String accessToken, String xuid, boolean microsoft) {
		//#if MC >= 26.1
		Minecraft mc = Minecraft.getInstance();
		if (mc.level != null) {
			return "leave the world first";
		}
		MinecraftAccountAccess access = (MinecraftAccountAccess) mc;
		User user = new User(name, uuid, accessToken, Optional.ofNullable(xuid).filter(x -> !"0".equals(x)),
				mc.getUser().getClientId());
		UserApiService api = UserApiService.OFFLINE;
		if (microsoft) {
			try {
				//#if MC >= 26.3
				api = MinecraftServicesDiscoveryService.create(access.arctic$proxy(), false).createUserApiService(accessToken);
				//#else
				api = new YggdrasilAuthenticationService(access.arctic$proxy()).createUserApiService(accessToken);
				//#endif
			} catch (Exception e) {
				ArcticMod.LOG.warn("account switch: user API for {}: {}", name, e.toString());
			}
		}
		final UserApiService service = api;
		access.arctic$setUser(user);
		access.arctic$setUserApiService(service);
		access.arctic$setUserPropertiesFuture(CompletableFuture.supplyAsync(() -> {
			try {
				return service.fetchProperties();
			} catch (Exception e) {
				return UserApiService.OFFLINE_PROPERTIES;
			}
		}, Util.nonCriticalIoPool()));
		access.arctic$setProfileFuture(CompletableFuture.supplyAsync(
				() -> mc.services().sessionService().fetchProfile(uuid, true), Util.nonCriticalIoPool()));
		access.arctic$setProfileKeyPairManager(ProfileKeyPairManager.create(service, user, mc.gameDirectory.toPath()));
		access.arctic$setReportingContext(ReportingContext.create(ReportEnvironment.local(), service));
		ArcticMod.LOG.info("Now playing as {}", name);
		return null;
		//#else
		return "not supported on this version yet";
		//#endif
	}
}
