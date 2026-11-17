package com.arcticlauncher.client.social;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import java.util.ArrayList;
import java.util.List;
import java.util.Collections;

import org.junit.jupiter.api.Test;

import com.google.gson.JsonObject;
import com.google.gson.JsonArray;

class InviteTargetTest {
	private JsonObject invite(String kind, String target) {
		JsonObject invite = new JsonObject();
		invite.addProperty("id", "invite-1");
		invite.addProperty("kind", kind);
		invite.addProperty("target", target);
		return invite;
	}

	@Test
	void togetherNoticeHandsOnlyTheCodeToLauncherJoin() {
		List<String> codes = new ArrayList<String>();
		InviteTarget target = InviteTarget.parse(invite("together", "  arctic-code  "));
		assertNotNull(target);
		assertTrue(target.join(codes::add, server -> { fail("Together code sent to server connect"); return false; }));
		assertEquals(Collections.singletonList("arctic-code"), codes);
	}

	@Test
	void serverNoticeUsesServerConnectAndKeepsThePort() {
		List<String> servers = new ArrayList<String>();
		InviteTarget target = InviteTarget.parse(invite("server", "mc.example.org:25566"));
		assertNotNull(target);
		assertTrue(target.join(code -> { fail("Server address sent to Together"); return false; }, servers::add));
		assertEquals(Collections.singletonList("mc.example.org:25566"), servers);
		assertFalse(target.join(code -> true, server -> false));
	}

	@Test
	void incompleteAndUnknownInvitesHaveNoJoinAction() {
		assertNull(InviteTarget.parse(null));
		assertNull(InviteTarget.parse(new JsonObject()));
		assertNull(InviteTarget.parse(invite("future", "target")));
		assertNull(InviteTarget.parse(invite("together", "  ")));
		JsonObject malformed = invite("server", "address");
		malformed.add("target", new JsonObject());
		assertNull(InviteTarget.parse(malformed));
		malformed = invite("server", "address");
		malformed.remove("id");
		assertNull(InviteTarget.parse(malformed));
	}

	@Test
	void staleNoticeCannotDispatchAJoin() {
		Social social = new Social(null, null);
		assertFalse(social.joinInvite(invite("server", "old.example.org"),
			code -> { fail("Stale together invite joined"); return true; },
			server -> { fail("Stale server invite joined"); return true; }));
	}

	private JsonObject overview(JsonObject invite) {
		JsonObject result = new JsonObject();
		JsonArray invites = new JsonArray();
		if (invite != null) {
			JsonObject from = new JsonObject();
			from.addProperty("name", "Alex");
			invite.add("from", from);
			invites.add(invite);
		}
		result.add("invites", invites);
		return result;
	}

	@Test
	void noticeAndTabHandlerConsumeSuccessfulInvitesOnlyOnce() {
		for (String kind : new String[] {"server", "together"}) {
			Social social = new Social(null, null);
			JsonObject invite = invite(kind, "destination");
			social.updateFriends(overview(invite));
			List<String> destinations = new ArrayList<String>();
			assertTrue(social.joinInvite(invite, destinations::add, destinations::add));
			assertEquals(Collections.singletonList("destination"), destinations);
			assertTrue(social.invites().isEmpty());
			// A failed dismissal on the service must not bring the same invite back locally.
			social.updateFriends(overview(invite));
			assertTrue(social.invites().isEmpty());
			assertFalse(social.joinInvite(invite, destinations::add, destinations::add));
			assertEquals(1, destinations.size());
		}
	}

	@Test
	void unavailableJoinCanBeRetriedAndExpiredInviteCannotBeUsed() {
		Social social = new Social(null, null);
		JsonObject invite = invite("together", "code");
		social.updateFriends(overview(invite));
		assertFalse(social.joinInvite(invite, code -> false, server -> false));
		assertEquals(1, social.invites().size());
		social.updateFriends(overview(null));
		assertFalse(social.joinInvite(invite, code -> { fail("Expired invite joined"); return true; }, server -> true));
	}

}
