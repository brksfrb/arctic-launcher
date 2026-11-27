package com.arcticlauncher.client.account;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.Platform;
import java.lang.reflect.Proxy;
import org.junit.jupiter.api.Test;

class AccountSwitcherTest {
	@Test
	void renewalSkipsTheBridgeForAnOfflinePlayer() {
		Platform offline = (Platform) Proxy.newProxyInstance(Platform.class.getClassLoader(),
				new Class<?>[] {Platform.class}, (proxy, method, args) -> {
					if ("offlineSession".equals(method.getName())) {
						return true;
					}
					throw new AssertionError("offline renewal called " + method.getName());
				});
		assertFalse(new AccountSwitcher(offline, 1, "", 0).renewSession());
	}

	@Test
	void onlyMicrosoftSessionsWithAUsableTokenCanBeRenewed() {
		assertFalse(AccountSwitcher.renewableSession("legacy", "tok"));
		assertFalse(AccountSwitcher.renewableSession("legacy", "0"));
		assertFalse(AccountSwitcher.renewableSession("msa", "0"));
		assertFalse(AccountSwitcher.renewableSession("msa", ""));
		assertFalse(AccountSwitcher.renewableSession("msa", null));
		assertTrue(AccountSwitcher.renewableSession("msa", "tok"));
	}
}
