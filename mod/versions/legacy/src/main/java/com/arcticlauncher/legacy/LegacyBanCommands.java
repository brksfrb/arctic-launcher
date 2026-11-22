package com.arcticlauncher.legacy;

import com.arcticlauncher.client.together.HostedBanCommands;
import com.arcticlauncher.client.together.HostedBanCommands.Action;
import com.arcticlauncher.client.together.WorldBanStore;
import com.mojang.authlib.GameProfile;
import io.netty.channel.local.LocalAddress;
import java.net.SocketAddress;
import java.util.Collections;
import java.util.List;
import java.util.UUID;
import net.minecraft.command.AbstractCommand;
import net.minecraft.command.CommandException;
import net.minecraft.command.CommandSource;
import net.minecraft.command.IncorrectUsageException;
import net.minecraft.entity.player.ServerPlayerEntity;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.OperatorEntry;
import net.minecraft.server.command.CommandManager;
import net.minecraft.server.integrated.IntegratedServer;
import net.minecraft.text.LiteralText;
import net.minecraft.util.math.BlockPos;

public final class LegacyBanCommands extends HostedBanCommands<ServerPlayerEntity> {
	private final IntegratedServer server;

	private LegacyBanCommands(IntegratedServer server) {
		super((WorldBanStore.Holder) server.getPlayerManager());
		this.server = server;
	}

	public static void register(CommandManager manager, IntegratedServer server) {
		for (Action action : Action.values()) {
			manager.registerCommand(new BanCommand(server, action));
		}
	}

	private static final class BanCommand extends AbstractCommand {
		private final IntegratedServer server;
		private final Action action;

		BanCommand(IntegratedServer server, Action action) {
			this.server = server;
			this.action = action;
		}

		@Override
		public String getCommandName() {
			return action.command;
		}

		@Override
		public String getUsageTranslationKey(CommandSource source) {
			switch (action) {
				case BAN: return "commands.ban.usage";
				case PARDON: return "commands.unban.usage";
				case BAN_IP: return "commands.banip.usage";
				case PARDON_IP: return "commands.unbanip.usage";
				case LIST: return "commands.banlist.usage";
				default: throw new AssertionError(action);
			}
		}

		@Override
		public int getPermissionLevel() {
			return 3;
		}

		@Override
		//#if MC <= 1.8.9
		public boolean isAccessible(CommandSource source) {
		//#else
		public boolean method_3278(MinecraftServer minecraft, CommandSource source) {
		//#endif
			if (!(source instanceof ServerPlayerEntity)) {
				return false;
			}
			ServerPlayerEntity player = (ServerPlayerEntity) source;
			if (address(player) instanceof LocalAddress) {
				return true;
			}
			OperatorEntry op = server.getPlayerManager().getOpList().get(player.getGameProfile());
			return op != null && op.getPermissionLevel() >= getPermissionLevel();
		}

		@Override
		//#if MC <= 1.8.9
		public void execute(CommandSource source, String[] args) throws CommandException {
		//#else
		public void method_3279(MinecraftServer minecraft, CommandSource source, String[] args) throws CommandException {
		//#endif
			String target = args.length == 0 ? null : args[0];
			boolean acceptsReason = action == Action.BAN || action == Action.BAN_IP;
			LegacyBanCommands commands = new LegacyBanCommands(server);
			if (action == Action.LIST) {
				if (args.length > 1 || target != null && !commands.suggestions(Action.LIST).contains(target)) {
					throw new IncorrectUsageException(getUsageTranslationKey(source));
				}
			} else if (target == null || target.isEmpty() || !acceptsReason && args.length != 1) {
				throw new IncorrectUsageException(getUsageTranslationKey(source));
			}
			String reason = args.length > 1 ? method_10706(args, 1) : null;
			try {
				commands.run(action, target, reason, message -> source.sendMessage(new LiteralText(message)));
			} catch (Rejected ex) {
				throw new CommandException("%s", new LiteralText(ex.getMessage()));
			}
		}

		@Override
		//#if MC <= 1.8.9
		public List<String> getAutoCompleteHints(CommandSource source, String[] args, BlockPos pos) {
		//#else
		public List<String> method_10738(MinecraftServer minecraft, CommandSource source, String[] args, BlockPos pos) {
		//#endif
			if (args.length != 1) {
				return Collections.emptyList();
			}
			return method_10708(args, new LegacyBanCommands(server).suggestions(action));
		}
	}

	private static SocketAddress address(ServerPlayerEntity player) {
		return player.networkHandler.getConnection().getAddress();
	}

	@Override
	protected List<ServerPlayerEntity> players() {
		return server.getPlayerManager().getPlayers();
	}

	@Override
	protected String name(ServerPlayerEntity player) {
		return player.getGameProfile().getName();
	}

	@Override
	protected UUID id(ServerPlayerEntity player) {
		return player.getGameProfile().getId();
	}

	@Override
	protected SocketAddress remote(ServerPlayerEntity player) {
		return address(player);
	}

	@Override
	protected Profile resolveProfile(String name) {
		GameProfile profile = server.getUserCache().findByName(name);
		return profile == null ? null : new Profile(profile.getName(), profile.getId());
	}

	@Override
	protected String hostName() {
		return server.getServerName();
	}

	@Override
	protected void disconnect(ServerPlayerEntity player, String message) {
		//#if MC >= 1.12
		player.networkHandler.method_14977(new LiteralText(message));
		//#else
		player.networkHandler.disconnect(message);
		//#endif
	}
}
