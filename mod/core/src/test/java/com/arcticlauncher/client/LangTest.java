package com.arcticlauncher.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import org.junit.jupiter.api.Test;

class LangTest {
	@Test
	void placeholdersAreFilledAndUnknownOnesKept() {
		assertEquals("Hi Steve, 3 left {x} {}", Lang.fill("Hi {0}, {1} left {x} {}", new Object[] {"Steve", 3}));
		assertEquals("no braces", Lang.fill("no braces", new Object[] {"a"}));
		assertEquals("open {brace", Lang.fill("open {brace", new Object[] {"a"}));
		assertEquals("{2} stays", Lang.fill("{2} stays", new Object[] {"a"}));
		assertEquals("same", Lang.fill("same", null));
	}

	@Test
	void pluralFormsFollowEachLanguage() {
		assertEquals("one", Lang.pluralForm("en", 1));
		assertEquals("other", Lang.pluralForm("en", 0));
		assertEquals("one", Lang.pluralForm("fr", 0));
		assertEquals("one", Lang.pluralForm("ru", 21));
		assertEquals("many", Lang.pluralForm("ru", 11));
		assertEquals("few", Lang.pluralForm("ru", 3));
		assertEquals("many", Lang.pluralForm("pl", 21));
		assertEquals("few", Lang.pluralForm("pl", 22));
		assertEquals("other", Lang.pluralForm("zh", 1));
	}

	@Test
	void languageCodesAreRecognisedInAnySpelling() {
		assertEquals("tr", Lang.supported("tr_TR"));
		assertEquals("pt", Lang.supported("pt_br"));
		assertEquals("zh", Lang.supported("zh_cn"));
		assertEquals("en", Lang.supported("EN-us"));
		assertNull(Lang.supported("xx_yy"));
		assertNull(Lang.supported(null));
		assertEquals("tr", Lang.choose("tr", "de_de"));
		assertEquals("de", Lang.choose("", "de_de"));
		assertEquals("ja", Lang.choose(null, "ja_jp"));
		assertEquals("en", Lang.choose(null, "xx_yy"));
	}

	@Test
	void englishIsTheFallbackAndKeysTheLastResort() {
		Lang.init(null, "en");
		assertFalse(Lang.load("en").isEmpty(), "assets/arctic/lang/en has text");
		assertEquals("no.such.key", Lang.t("no.such.key"));
		assertNull(Lang.lookup("no.such.key"));
	}

	/** Every translation has a key English has, with the same placeholders. */
	@Test
	void translationsMatchEnglish() {
		Map<String, String> english = Lang.load("en");
		List<String> problems = new ArrayList<String>();
		for (String code : Lang.CODES) {
			Map<String, String> strings = Lang.load(code);
			assertFalse(strings.isEmpty(), "assets/arctic/lang/" + code + " has text");
			for (Map.Entry<String, String> e : strings.entrySet()) {
				if (e.getValue().trim().isEmpty()) {
					problems.add(code + ": " + e.getKey() + " is empty");
				}
				if (code.equals("en")) {
					continue;
				}
				String source = english.get(e.getKey());
				if (source == null) {
					problems.add(code + ": " + e.getKey() + " is not in English");
				} else if (!placeholders(source).equals(placeholders(e.getValue()))) {
					problems.add(code + ": " + e.getKey() + " has other placeholders than English");
				}
			}
		}
		assertTrue(problems.isEmpty(), String.join("\n", problems));
	}

	private static TreeSet<String> placeholders(String s) {
		TreeSet<String> out = new TreeSet<String>();
		int i = 0;
		while ((i = s.indexOf('{', i)) >= 0) {
			int end = s.indexOf('}', i);
			if (end < 0) {
				break;
			}
			out.add(s.substring(i + 1, end));
			i = end + 1;
		}
		return out.isEmpty() ? new TreeSet<String>(Collections.<String>emptySet()) : out;
	}
}
