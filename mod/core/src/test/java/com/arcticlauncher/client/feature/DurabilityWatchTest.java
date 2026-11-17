package com.arcticlauncher.client.feature;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.arcticlauncher.client.config.ClientConfig;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Test;

class DurabilityWatchTest {
	private static Object[] row(String name, int left, int max, String slot, String tier) {
		return new Object[] {name, left, max, slot, tier};
	}

	private static List<Object[]> holding(Object[]... rows) {
		return new ArrayList<Object[]>(Arrays.asList(rows));
	}

	@Test
	void thresholdIsInclusiveAndFollowsThePercent() {
		assertTrue(DurabilityWatch.low(10, 100, 10));
		assertFalse(DurabilityWatch.low(11, 100, 10));
		assertTrue(DurabilityWatch.low(156, 1561, 10)); // 156*100 = 15600 <= 15610
		assertFalse(DurabilityWatch.low(157, 1561, 10));
		assertTrue(DurabilityWatch.low(25, 100, 25));
		assertFalse(DurabilityWatch.low(26, 100, 25));
		assertFalse(DurabilityWatch.low(0, 0, 10), "no max, nothing to wear out");
		assertTrue(DurabilityWatch.low(0, 59, 1));
	}

	@Test
	void percentIsClamped() {
		assertEquals(1, DurabilityWatch.clampPercent(0));
		assertEquals(1, DurabilityWatch.clampPercent(-5));
		assertEquals(50, DurabilityWatch.clampPercent(90));
		assertEquals(10, DurabilityWatch.clampPercent(10));
		assertTrue(DurabilityWatch.low(1, 100, 0), "0 counts as the lowest step");
	}

	@Test
	void materialFilter() {
		assertEquals("diamond", DurabilityWatch.tierOf("diamond_pickaxe"));
		assertEquals("netherite", DurabilityWatch.tierOf("netherite_chestplate"));
		assertEquals("other", DurabilityWatch.tierOf("iron_sword"));
		assertEquals("other", DurabilityWatch.tierOf("golden_diamond")); // only a prefix counts
		assertEquals("other", DurabilityWatch.tierOf(null));
		assertTrue(DurabilityWatch.watched("other", false));
		assertFalse(DurabilityWatch.watched("other", true));
		assertTrue(DurabilityWatch.watched("diamond", true));
		assertTrue(DurabilityWatch.watched("netherite", true));
		assertFalse(DurabilityWatch.watched(null, true));
	}

	@Test
	void oldConfigsKeepTenPercentAndDoNotBlock() {
		ClientConfig c = new ClientConfig();
		assertTrue(c.durabilityWarning);
		assertEquals(10, c.durabilityPercent);
		assertFalse(c.durabilityBlockUse);
		assertFalse(c.durabilityDiamondOnly);
	}

	@Test
	void warnsOnceUntilRepaired() {
		ClientConfig c = new ClientConfig();
		DurabilityWatch watch = new DurabilityWatch();
		List<Object[]> rows = holding(row("Pick", 5, 100, "mainhand", "iron"));
		assertEquals(1, watch.scan(rows, c).size());
		assertEquals(0, watch.scan(rows, c).size(), "same item, no second pop-up");
		rows.set(0, row("Pick", 4, 100, "mainhand", "iron"));
		assertEquals(0, watch.scan(rows, c).size(), "still low as it wears");
		rows.set(0, row("Pick", 100, 100, "mainhand", "iron"));
		assertEquals(0, watch.scan(rows, c).size(), "repaired");
		rows.set(0, row("Pick", 9, 100, "mainhand", "iron"));
		assertEquals(1, watch.scan(rows, c).size(), "low again after the repair");
	}

	@Test
	void popupUsesThePercentAndTheFilter() {
		ClientConfig c = new ClientConfig();
		List<Object[]> rows = holding(row("Pick", 20, 100, "mainhand", "other"), row("Helmet", 20, 100, "head", "diamond"));
		assertEquals(0, new DurabilityWatch().scan(rows, c).size(), "20% is above the default 10%");
		c.durabilityPercent = 20;
		assertEquals(2, new DurabilityWatch().scan(rows, c).size());
		c.durabilityDiamondOnly = true;
		List<Object[]> fresh = new DurabilityWatch().scan(rows, c);
		assertEquals(1, fresh.size());
		assertEquals("Helmet", fresh.get(0)[0]);
	}

	@Test
	void useBlockOnlyForTheHeldItemBelowTheThreshold() {
		ClientConfig c = new ClientConfig();
		List<Object[]> rows = holding(row("Sword", 5, 100, "mainhand", "other"));
		assertFalse(DurabilityWatch.useBlocked(c, rows), "off by default");
		c.durabilityBlockUse = true;
		assertTrue(DurabilityWatch.useBlocked(c, rows));
		c.durabilityWarning = false;
		assertFalse(DurabilityWatch.useBlocked(c, rows), "the tile is the master switch");
		c.durabilityWarning = true;
		assertFalse(DurabilityWatch.useBlocked(c, holding(row("Sword", 11, 100, "mainhand", "other"))));
		assertFalse(DurabilityWatch.useBlocked(c, holding(row("Helmet", 1, 100, "head", "diamond"))), "armor is never blocked");
		assertFalse(DurabilityWatch.useBlocked(c, holding(row("Shield", 1, 100, "offhand", "other"))));
		assertFalse(DurabilityWatch.useBlocked(c, holding()), "non-damageable items have no row");
		assertFalse(DurabilityWatch.useBlocked(c, (com.arcticlauncher.client.Platform) null));
		c.durabilityDiamondOnly = true;
		assertFalse(DurabilityWatch.useBlocked(c, rows), "iron ignored by the filter");
		assertTrue(DurabilityWatch.useBlocked(c, holding(row("Axe", 5, 100, "mainhand", "netherite"))));
	}
}
