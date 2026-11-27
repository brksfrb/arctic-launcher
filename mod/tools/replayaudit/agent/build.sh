#!/bin/sh
# build.sh <outdir>: auditagent.jar (premain) + auditclock.jar (bootstrap clock) for Java 8 and newer.
set -e
OUT="$1"; HERE="$(cd "$(dirname "$0")" && pwd)"
ASM="$(ls /home/claude/fl-run/data/shared/libraries/org/ow2/asm/asm/9.*/asm-9.*.jar | tail -1)"
JAVAC="${JAVA_HOME:-/usr/lib/jvm/java-25-openjdk-amd64}/bin/javac"; JAR="${JAVA_HOME:-/usr/lib/jvm/java-25-openjdk-amd64}/bin/jar"
rm -rf "$OUT/build"; mkdir -p "$OUT/build/a" "$OUT/build/c"
"$JAVAC" --release 8 -nowarn -d "$OUT/build/c" "$HERE/src/auditagent/A.java"
"$JAVAC" --release 8 -nowarn -cp "$ASM:$OUT/build/c" -d "$OUT/build/a" "$HERE/src/auditagent/Agent.java"
printf 'Premain-Class: auditagent.Agent\n' > "$OUT/build/MANIFEST.MF"
"$JAR" cfm "$OUT/auditagent.jar" "$OUT/build/MANIFEST.MF" -C "$OUT/build/a" .
"$JAR" cf "$OUT/auditclock.jar" -C "$OUT/build/c" .
