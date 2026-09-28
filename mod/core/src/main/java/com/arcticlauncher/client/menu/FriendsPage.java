package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** Friends as a page: who's online and where, chat, screenshots, voice, duels. */
final class FriendsPage implements MenuTab {
	private final FriendsTab friends = new FriendsTab();
	private int x;
	private int top;
	private int w;

	@Override
	public String title() {
		return "Friends";
	}

	@Override
	public String hint() {
		return "Who's online and where; chat and send screenshots.";
	}

	@Override
	public void build(Host host, int x, int top, int w, int bottom) {
		this.x = x;
		this.top = top;
		this.w = w;
		friends.build(host, x, top, w, bottom);
	}

	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		friends.draw(g, s, x, top, w);
	}

	@Override
	public String state() {
		return FriendsTab.state();
	}
}
