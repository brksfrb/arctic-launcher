package com.arcticlauncher.bedrock;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class WorldStateTest {
    private static WorldFrame frame(String session,long sequence) {
        WorldFrame frame=new WorldFrame();frame.session=session;frame.sequence=sequence;return frame;
    }
    @Test void temporaryDepthGapRetainsPlaybackSessionButNeverReusesItsRenderFrame() {
        WorldState state=new WorldState();WorldFrame first=frame("one",1);
        assertTrue(state.frame(first));assertTrue(state.renderable);
        assertFalse(state.gap(true));assertSame(first,state.current);assertFalse(state.renderable);
        assertFalse(state.frame(frame("one",2)));assertTrue(state.renderable);
        assertFalse(state.gap(true));assertTrue(state.frame(frame("two",1)));
        assertTrue(state.gap(false));assertNull(state.current);assertFalse(state.renderable);
        assertFalse(state.gap(false));
    }
    @Test void explicitClearAndSessionChangesStillResetPlayback() {
        WorldState state=new WorldState();
        assertFalse(state.frame(null));
        assertTrue(state.frame(frame("one",1)));
        assertTrue(state.frame(null));assertNull(state.current);assertFalse(state.renderable);
        assertTrue(state.frame(frame("one",2)));
    }
}
