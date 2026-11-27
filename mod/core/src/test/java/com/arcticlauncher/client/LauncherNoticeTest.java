package com.arcticlauncher.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.fail;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Test;

class LauncherNoticeTest {
	@Test
	void launcherVersionSuppressesTheNotice() {
		LauncherNotice notice = new LauncherNotice("0.6.5");
		notice.show((title, body) -> fail("Arctic Launcher starts should be silent"));
	}

	@Test
	void missingLauncherVersionShowsTheNoticeOnce() {
		Lang.init(null, "en");
		LauncherNotice notice = new LauncherNotice(null);
		List<List<String>> posted = new ArrayList<>();
		notice.show((title, body) -> posted.add(Arrays.asList(title, body)));
		notice.show((title, body) -> fail("The notice already appeared this session"));

		assertEquals(1, posted.size());
		assertEquals(Arrays.asList("Outside Arctic Launcher",
				"Sign-in, cosmetics and sync may be limited. arcticlauncher.com"), posted.get(0));
	}
}
