package com.arcticlauncher.client.replay;

import static org.junit.jupiter.api.Assertions.*;
import java.io.*;
import java.util.*;
import org.junit.jupiter.api.Test;

class AudioEventTest {
	private AudioEvent event(int id, int time, String resource) {
		AudioEvent e = new AudioEvent(); e.id = id; e.time = time; e.resource = resource;
		e.left = .5f; e.right = .7f; return e;
	}
	@Test void clipKeepsMusicPositionAndDropsStoppedSounds() throws Exception {
		AudioEvent music = event(1, 0, "minecraft:sounds/music/game.ogg"); music.pitch = 2;
		AudioEvent effect = event(2, 500, "minecraft:sounds/effect.ogg");
		AudioEvent stop = event(2, 1000, "");
		AudioEvent later = event(3, 3500, "minecraft:sounds/later.ogg");
		ByteArrayOutputStream source = new ByteArrayOutputStream();
		for (AudioEvent e : Arrays.asList(music, effect, stop, later)) e.write(new DataOutputStream(source));
		ByteArrayOutputStream result = new ByteArrayOutputStream();
		AudioEvent.copy(new DataInputStream(new ByteArrayInputStream(source.toByteArray())), 4,
				new DataOutputStream(result), 3000, 4000, 3000, n -> assertEquals(2, n));
		DataInputStream in = new DataInputStream(new ByteArrayInputStream(result.toByteArray()));
		AudioEvent first = AudioEvent.read(in); assertEquals(1, first.id); assertEquals(0, first.time); assertEquals(6, first.position);
		assertEquals(500, AudioEvent.read(in).time); assertEquals(0, in.available());
	}
	@Test void emptyRangeStillSeedsCurrentSound() throws Exception {
		AudioEvent music = event(1, 100, "sound");
		ByteArrayOutputStream source = new ByteArrayOutputStream(); music.write(new DataOutputStream(source));
		ByteArrayOutputStream result = new ByteArrayOutputStream();
		AudioEvent.copy(new DataInputStream(new ByteArrayInputStream(source.toByteArray())), 1,
				new DataOutputStream(result), 5000, 5000, 5000, n -> assertEquals(1, n));
		assertEquals(4.9, AudioEvent.read(new DataInputStream(new ByteArrayInputStream(result.toByteArray()))).position, .00001);
	}
}
