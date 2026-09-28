package com.arcticlauncher.client.ui;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class TextFieldTest {
	@Test
	void ctrlBackspaceRemovesTheLastWordAndTheSpacesAfterIt() {
		assertEquals(6, TextField.wordStart("hello world"));
		assertEquals(6, TextField.wordStart("hello world  "));
		assertEquals(0, TextField.wordStart("hello"));
		assertEquals(0, TextField.wordStart(""));
	}
}
