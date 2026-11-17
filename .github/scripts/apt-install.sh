#!/usr/bin/env bash
# Install apt packages on GitHub-hosted Ubuntu runners, resilient to ubuntu.com DNS outages.
# Usage: apt-install.sh [--no-recommends] pkg...
# - No-op if every package is already installed.
# - Retries apt fetches.
# - If the Ubuntu archive hosts do not resolve (or `apt-get update` fails), points apt at
#   a mirror on another domain. apt still verifies Ubuntu archive signatures.
set -euo pipefail

MIRROR="${APT_FALLBACK_MIRROR:-https://mirrors.edge.kernel.org/ubuntu}"
flags=(-y)
if [ "${1:-}" = "--no-recommends" ]; then flags+=(--no-install-recommends); shift; fi
[ "$#" -gt 0 ] || { echo "usage: $0 [--no-recommends] pkg..." >&2; exit 2; }

missing=()
for p in "$@"; do
  dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q '^install ok installed' || missing+=("$p")
done
if [ "${#missing[@]}" -eq 0 ]; then
  echo "apt-install: already installed: $*"
  exit 0
fi

SUDO=""; [ "$(id -u)" -eq 0 ] || SUDO="sudo"
$SUDO tee /etc/apt/apt.conf.d/80-ci-retries >/dev/null <<'CONF'
Acquire::Retries "5";
Acquire::http::Timeout "20";
Acquire::https::Timeout "20";
CONF

use_mirror() {
  echo "apt-install: switching apt sources to $MIRROR"
  local f
  for f in /etc/apt/sources.list /etc/apt/sources.list.d/*.sources /etc/apt/sources.list.d/*.list; do
    [ -f "$f" ] || continue
    $SUDO sed -i -E "s#https?://([a-z0-9-]+\.)?(archive|security)\.ubuntu\.com/ubuntu/?#${MIRROR%/}/#g" "$f"
  done
}

dns_ok=1
for h in azure.archive.ubuntu.com archive.ubuntu.com security.ubuntu.com; do
  getent hosts "$h" >/dev/null 2>&1 || { echo "apt-install: $h does not resolve"; dns_ok=0; }
done

[ "$dns_ok" -eq 1 ] || use_mirror
if ! $SUDO apt-get update; then
  echo "apt-install: apt-get update failed; retrying via fallback mirror"
  use_mirror
  $SUDO apt-get update
fi
$SUDO apt-get install "${flags[@]}" "${missing[@]}"
