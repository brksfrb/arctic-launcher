package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.account.AccountSwitcher;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** Accounts and the proxy as a page. */
final class AccountPage implements MenuTab {
	private final AccountTab account = new AccountTab();
	private int x;
	private int w;

	@Override
	public String title() {
		return "Account";
	}

	@Override
	public String hint() {
		return "Switch accounts, or connect through a proxy.";
	}

	@Override
	public void build(Host host, int x, int top, int w, int bottom) {
		this.x = x;
		this.w = w;
		account.build(host, x, top, w);
	}

	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		account.draw(g, s, x, w);
	}

	@Override
	public String state() {
		AccountSwitcher s = ArcticClient.accounts();
		if (s == null) {
			return "";
		}
		StringBuilder b = new StringBuilder().append(s.busy()).append('/').append(s.signingIn()).append('/')
				.append(ArcticClient.platform().playerName());
		for (AccountSwitcher.Entry e : s.accounts()) {
			b.append('/').append(e.id);
		}
		return b.toString();
	}
}
