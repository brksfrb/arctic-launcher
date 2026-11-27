//#if MC >= 1.18
package com.arcticlauncher.mod.startup;

/** Install before Mixin selection, while the agent is starting. */
final class ExitTrace {
    private ExitTrace() {}
    static void install() { com.arcticlauncher.client.CrashDiagnostics.install(); }
}
//#endif
