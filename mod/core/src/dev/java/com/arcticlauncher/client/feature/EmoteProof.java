package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import java.io.PrintWriter;
import java.util.HashSet;
import java.util.Set;

/** Opt-in offline-game proof; never packaged in releases. */
public final class EmoteProof {
	public static final String INCOMING = "Vanilla pasted :sob: :ice: :ice_cube: :iceberg: :bear:";
	public static final String WRAP = "Adjacent :sob::heart::star: :SOB: :stuck_out_tongue_winking: "
			+ ":arctic: :snowflake: :unknown: end. :sob::sob::sob::sob::sob::sob::sob::sob::sob::sob::sob::sob:";
	private EmoteProof() {}
	public static void check(ChatCopyProof.Game game, PrintWriter log) {
		Set<String> found = new HashSet<String>();
		for (int y = Math.max(0, game.height() - 230); y < game.height() - 25; y += 2) {
			for (int x = 3; x < Math.min(350, game.width()); x += 3) {
				Object style = game.hit(x, y);
				ChatCopy.Data data = ChatCopy.data(style);
				if (data == null || !data.text.contains(":sob:")) continue;
				if (found.add(data.text)) {
					ArcticClient.platform().setClipboard("before");
					if (!game.click(data.target ? 0 : 1, x, y, !data.target)) throw new AssertionError("copy hit");
					if (!data.text.equals(ArcticClient.platform().clipboard())) throw new AssertionError("source copy mismatch");
					log.println("PASS actual chat hit + clipboard retained wire source: " + data.text);
				}
			}
		}
		if (!found.contains(INCOMING) || !found.contains(WRAP) || found.size() < 3) throw new AssertionError("sent/incoming/wrapped lines missing: " + found);
		log.println("PASS pasted vanilla source and actual sent local-server echo, timestamps and copy");
		log.flush();
	}
}
