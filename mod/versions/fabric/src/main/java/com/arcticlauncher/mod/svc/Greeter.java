//#if MC >= 1.20.5
package com.arcticlauncher.mod.svc;

import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.network.Connection;
import net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket;

/**
 * Says hello to a server that lists a hello channel: Arctic's own ({@code arctic:hello}) and Polarium's
 * ({@code polarium:hello}, on Polarium's behalf), once per connection.
 *
 * <p>The server's channel list arrives in {@code minecraft:register}. Without Fabric API the client reads it
 * itself ({@link SvcPayload}); with Fabric API's networking installed, that module owns the message, so the
 * list comes from its channel events instead ({@link #listenToFabricApi}). Without this second path nobody
 * said hello on instances that have Fabric API, and servers thought Polarium wasn't there.
 */
public final class Greeter {
	private static Connection greeted;
	private static final Set<String> GREETINGS = new HashSet<>();

	private Greeter() {}

	/** The server on {@code connection} announced these channels. */
	public static synchronized void greet(Connection connection, List<String> channels) {
		if (connection == null || channels.isEmpty()) {
			return;
		}
		if (connection != greeted) {
			greeted = connection;
			GREETINGS.clear();
		}
		if (channels.contains(SvcPayload.HELLO.toString()) && GREETINGS.add("arctic")) {
			connection.send(new ServerboundCustomPayloadPacket(SvcPayload.hello()));
		}
		if (channels.contains(SvcPayload.POLARIUM_HELLO.toString()) && GREETINGS.add("polarium")) {
			SvcPayload hello = SvcPayload.polariumHello();
			if (hello != null) {
				connection.send(new ServerboundCustomPayloadPacket(hello));
			}
		}
	}

	/** With Fabric API's networking installed, its channel events carry the server's channel list. */
	public static void listenToFabricApi() {
		if (!FabricLoader.getInstance().isModLoaded("fabric-networking-api-v1")) {
			return;
		}
		hook("net.fabricmc.fabric.api.client.networking.v1.C2SConfigurationChannelEvents");
		hook("net.fabricmc.fabric.api.client.networking.v1.C2SPlayChannelEvents");
	}

	/** {@code <events>.REGISTER.register((handler, sender, client, channels) -> greet(...))}, by name. */
	private static void hook(String events) {
		try {
			Class<?> owner = Class.forName(events);
			Object event = owner.getField("REGISTER").get(null);
			Class<?> callback = Class.forName(events + "$Register");
			Object listener = Proxy.newProxyInstance(callback.getClassLoader(), new Class<?>[] {callback}, (proxy, method, args) -> {
				if (method.getDeclaringClass() == Object.class) {
					return switch (method.getName()) {
						case "hashCode" -> System.identityHashCode(proxy);
						case "equals" -> proxy == args[0];
						default -> "Arctic hello listener";
					};
				}
				if (args != null && args.length == 4 && args[3] instanceof List<?> ids) {
					List<String> channels = new ArrayList<>(ids.size());
					for (Object id : ids) {
						channels.add(String.valueOf(id));
					}
					greet(connectionOf(args[0]), channels);
				}
				return null;
			});
			Method register = Class.forName("net.fabricmc.fabric.api.event.Event").getMethod("register", Object.class);
			register.invoke(event, listener);
		} catch (ReflectiveOperationException | RuntimeException | LinkageError e) {
			// Another Fabric API shape: no hello through it.
		}
	}

	private static Connection connectionOf(Object handler) {
		try {
			Object connection = handler.getClass().getMethod("getConnection").invoke(handler);
			return connection instanceof Connection c ? c : null;
		} catch (ReflectiveOperationException | RuntimeException e) {
			return null;
		}
	}
}
//#endif
