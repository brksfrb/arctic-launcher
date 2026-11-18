//#if MC >= 1.14
package com.arcticlauncher.mod;

import com.arcticlauncher.client.together.HostedBanCommands;
import com.arcticlauncher.client.together.HostedBanCommands.Action;
import com.arcticlauncher.client.together.WorldBanStore;
import com.arcticlauncher.mod.mixin.CommandSourceStackAccess;
//#if MC < 1.21.9
import com.mojang.authlib.GameProfile;
//#endif
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.StringArgumentType;
import com.mojang.brigadier.builder.LiteralArgumentBuilder;
import com.mojang.brigadier.builder.RequiredArgumentBuilder;
import com.mojang.brigadier.context.CommandContext;
import com.mojang.brigadier.exceptions.CommandSyntaxException;
import com.mojang.brigadier.exceptions.SimpleCommandExceptionType;
import io.netty.channel.local.LocalAddress;
import java.net.SocketAddress;
import java.util.List;
import java.util.UUID;
import net.minecraft.client.server.IntegratedServer;
import net.minecraft.commands.CommandSource;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.commands.Commands;
import net.minecraft.commands.SharedSuggestionProvider;
import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerPlayer;
//#if MC >= 1.21.9
import net.minecraft.server.players.NameAndId;
//#endif
import net.minecraft.server.players.ServerOpListEntry;

public final class IntegratedBanCommands extends HostedBanCommands<ServerPlayer> {
	private final IntegratedServer server;

	private IntegratedBanCommands(IntegratedServer server) {
		super((WorldBanStore.Holder) server.getPlayerList());
		this.server = server;
	}

	public static void register(CommandDispatcher<CommandSourceStack> dispatcher) {
		dispatcher.register(root(Action.BAN)
				.then(target(Action.BAN, StringArgumentType.word())
						.executes(ctx -> run(ctx.getSource(), Action.BAN, value(ctx), null))
						.then(Commands.argument("reason", StringArgumentType.greedyString())
								.executes(ctx -> run(ctx.getSource(), Action.BAN, value(ctx),
										StringArgumentType.getString(ctx, "reason"))))));
		dispatcher.register(root(Action.PARDON)
				.then(target(Action.PARDON, StringArgumentType.word())
						.executes(ctx -> run(ctx.getSource(), Action.PARDON, value(ctx), null))));
		// A greedy argument accepts bare IPv6 literals. Only the first word is the target.
		dispatcher.register(root(Action.BAN_IP)
				.then(target(Action.BAN_IP, StringArgumentType.greedyString()).executes(ctx -> {
					String value = value(ctx);
					int space = value.indexOf(' ');
					String ip = space < 0 ? value : value.substring(0, space);
					String reason = space < 0 ? "" : value.substring(space + 1).trim();
					return run(ctx.getSource(), Action.BAN_IP, ip, reason.isEmpty() ? null : reason);
				})));
		dispatcher.register(root(Action.PARDON_IP)
				.then(target(Action.PARDON_IP, StringArgumentType.greedyString())
						.executes(ctx -> run(ctx.getSource(), Action.PARDON_IP, value(ctx).trim(), null))));
		dispatcher.register(root(Action.LIST).executes(ctx -> run(ctx.getSource(), Action.LIST, null, null))
				.then(Commands.literal("players").executes(ctx -> run(ctx.getSource(), Action.LIST, "players", null)))
				.then(Commands.literal("ips").executes(ctx -> run(ctx.getSource(), Action.LIST, "ips", null))));
	}

	private static LiteralArgumentBuilder<CommandSourceStack> root(Action action) {
		return Commands.literal(action.command).requires(IntegratedBanCommands::allowed);
	}

	private static RequiredArgumentBuilder<CommandSourceStack, String> target(Action action, StringArgumentType type) {
		return Commands.argument("target", type).suggests((ctx, builder) ->
				SharedSuggestionProvider.suggest(commands(ctx.getSource()).suggestions(action), builder));
	}

	private static String value(CommandContext<CommandSourceStack> ctx) {
		return StringArgumentType.getString(ctx, "target");
	}

	private static IntegratedBanCommands commands(CommandSourceStack source) {
		return new IntegratedBanCommands((IntegratedServer) source.getServer());
	}

	private static int run(CommandSourceStack source, Action action, String target, String reason)
			throws CommandSyntaxException {
		try {
			return commands(source).run(action, target, reason, message -> {
				Component text = Compat.literal(message);
				//#if MC >= 1.20
				source.sendSuccess(() -> text, action != Action.LIST);
				//#else
				source.sendSuccess(text, action != Action.LIST);
				//#endif
			});
		} catch (Rejected ex) {
			throw new SimpleCommandExceptionType(Compat.literal(ex.getMessage())).create();
		}
	}

	private static boolean allowed(CommandSourceStack source) {
		if (!(source.getServer() instanceof IntegratedServer) || !(source.getEntity() instanceof ServerPlayer)) {
			return false;
		}
		ServerPlayer player = (ServerPlayer) source.getEntity();
		// The executing entity can change in a vanilla command. Check who submitted it as well.
		CommandSource origin = ((CommandSourceStackAccess) (Object) source).arctic$source();
		//#if MC >= 1.21.2
		if (origin != player.commandSource()) {
		//#else
		if (origin != player) {
		//#endif
			return false;
		}
		if (address(player) instanceof LocalAddress) {
			return true;
		}
		//#if MC >= 1.21.9
		ServerOpListEntry op = source.getServer().getPlayerList().getOps().get(new NameAndId(player.getGameProfile()));
		//#else
		ServerOpListEntry op = source.getServer().getPlayerList().getOps().get(player.getGameProfile());
		//#endif
		//#if MC >= 1.21.11
		return op != null && Commands.LEVEL_ADMINS.check(op.permissions());
		//#else
		return op != null && op.getLevel() >= 3;
		//#endif
	}

	private static SocketAddress address(ServerPlayer player) {
		//#if MC >= 1.19.4
		return player.connection.getRemoteAddress();
		//#else
		return player.connection.connection.getRemoteAddress();
		//#endif
	}

	@Override
	protected List<ServerPlayer> players() {
		return server.getPlayerList().getPlayers();
	}

	@Override
	protected String name(ServerPlayer player) {
		return Compat.profileName(player.getGameProfile());
	}

	@Override
	protected UUID id(ServerPlayer player) {
		return Compat.profileId(player.getGameProfile());
	}

	@Override
	protected SocketAddress remote(ServerPlayer player) {
		return address(player);
	}

	@Override
	protected Profile resolveProfile(String name) {
		//#if MC >= 1.21.9
		return server.services().nameToIdCache().get(name).map(p -> new Profile(p.name(), p.id())).orElse(null);
		//#elif MC >= 1.17
		GameProfile profile = server.getProfileCache().get(name).orElse(null);
		//#else
		GameProfile profile = server.getProfileCache().get(name);
		//#endif
		//#if MC < 1.21.9
		return profile == null ? null : new Profile(Compat.profileName(profile), Compat.profileId(profile));
		//#endif
	}

	@Override
	protected String hostName() {
		//#if MC >= 1.19
		return Compat.profileName(server.getSingleplayerProfile());
		//#else
		return server.getSingleplayerName();
		//#endif
	}

	@Override
	protected void disconnect(ServerPlayer player, String message) {
		player.connection.disconnect(Compat.literal(message));
	}
}
//#endif
