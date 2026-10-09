package com.arcticlauncher.mod;

import com.arcticlauncher.mod.mixin.MinecraftAccountAccess;
import java.util.UUID;
import net.minecraft.client.Minecraft;
import net.minecraft.client.User;
//#if MC >= 1.18
import com.mojang.authlib.minecraft.UserApiService;
import java.util.Optional;
//#endif
//#if MC >= 26.3
import com.mojang.authlib.services.MinecraftServicesDiscoveryService;
//#elif MC >= 1.18
import com.mojang.authlib.yggdrasil.YggdrasilAuthenticationService;
//#endif
//#if MC >= 1.19
import net.minecraft.client.multiplayer.ProfileKeyPairManager;
import net.minecraft.client.multiplayer.chat.report.ReportEnvironment;
import net.minecraft.client.multiplayer.chat.report.ReportingContext;
//#endif
//#if MC >= 1.20.2
import java.util.concurrent.CompletableFuture;
//#endif

/**
 * Plays as another account without restarting (in game, or when a server says the session ran
 * out): the per-account parts Minecraft sets up at start (the user and, by version, its API
 * service and properties, profile, chat signing keys, reporting) are made again for the new one.
 */
public final class AccountSwap {
	private AccountSwap() {}

	/** Returns why it couldn't, or null. Main thread, outside a world. */
	public static String swap(String name, UUID uuid, String accessToken, String xuid, boolean microsoft) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.level != null) {
			return "leave the world first";
		}
		MinecraftAccountAccess access = (MinecraftAccountAccess) mc;
		User user = user(mc, name, uuid, accessToken, xuid, microsoft);
		access.arctic$setUser(user);
		//#if MC >= 1.18
		final UserApiService service = apiService(access, name, accessToken, microsoft);
		access.arctic$setUserApiService(service);
		//#endif
		//#if MC >= 1.20.3
		access.arctic$setUserPropertiesFuture(CompletableFuture.supplyAsync(() -> {
			try {
				return service.fetchProperties();
			} catch (Exception e) {
				return UserApiService.OFFLINE_PROPERTIES;
			}
		}, io()));
		//#endif
		//#if MC >= 1.20.2
		access.arctic$setProfileFuture(CompletableFuture.supplyAsync(
				//#if MC >= 1.21.9
				() -> mc.services().sessionService().fetchProfile(uuid, true), io()));
				//#else
				() -> mc.getMinecraftSessionService().fetchProfile(uuid, true), io()));
				//#endif
		//#endif
		//#if MC >= 1.19.3
		access.arctic$setProfileKeyPairManager(ProfileKeyPairManager.create(service, user, mc.gameDirectory.toPath()));
		//#elif MC >= 1.19
		access.arctic$setProfileKeyPairManager(new ProfileKeyPairManager(service, uuid, mc.gameDirectory.toPath()));
		//#endif
		//#if MC >= 1.19
		access.arctic$setReportingContext(ReportingContext.create(ReportEnvironment.local(), service));
		//#endif
		ArcticMod.LOG.info("Now playing as {}", name);
		return null;
	}

	private static User user(Minecraft mc, String name, UUID uuid, String token, String xuid, boolean microsoft) {
		//#if MC >= 1.21.9
		return new User(name, uuid, token, Optional.ofNullable(xuid).filter(x -> !"0".equals(x)), mc.getUser().getClientId());
		//#elif MC >= 1.20.2
		return new User(name, uuid, token, Optional.ofNullable(xuid).filter(x -> !"0".equals(x)), mc.getUser().getClientId(),
				microsoft ? User.Type.MSA : User.Type.LEGACY);
		//#elif MC >= 1.18
		return new User(name, uuid.toString().replace("-", ""), token, Optional.ofNullable(xuid).filter(x -> !"0".equals(x)),
				mc.getUser().getClientId(), microsoft ? User.Type.MSA : User.Type.LEGACY);
		//#else
		return new User(name, uuid.toString().replace("-", ""), token, microsoft ? "msa" : "legacy");
		//#endif
	}

	//#if MC >= 1.18
	private static UserApiService apiService(MinecraftAccountAccess access, String name, String token, boolean microsoft) {
		if (!microsoft) {
			return UserApiService.OFFLINE;
		}
		try {
			//#if MC >= 26.3
			return MinecraftServicesDiscoveryService.create(access.arctic$proxy(), false).createUserApiService(token);
			//#else
			return new YggdrasilAuthenticationService(access.arctic$proxy()).createUserApiService(token);
			//#endif
		} catch (Exception e) {
			ArcticMod.LOG.warn("account switch: user API for {}: {}", name, e.toString());
			return UserApiService.OFFLINE;
		}
	}
	//#endif

	//#if MC >= 1.20.2
	private static java.util.concurrent.Executor io() {
		//#if MC >= 1.21.11
		return net.minecraft.util.Util.nonCriticalIoPool();
		//#elif MC >= 1.20.4
		return net.minecraft.Util.nonCriticalIoPool();
		//#else
		return net.minecraft.Util.backgroundExecutor();
		//#endif
	}
	//#endif
}
