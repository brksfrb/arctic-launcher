#!/usr/bin/env bash
set -euo pipefail

if (( $# < 1 || $# > 2 )); then
    echo "Usage: $0 SIGNED_EXE [SHA256SUMS-unsigned.txt]" >&2
    exit 2
fi

exe=$1
case $exe in
    /*) ;;
    *) exe=$PWD/$exe ;;
esac
name=${exe##*/}
directory=${exe%/*}
checksums=${2:-"$directory/SHA256SUMS-unsigned.txt"}
normalized=$exe.unsigned

work=$(mktemp -d "$directory/.arctic-verify.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
stripped=$work/stripped
osslsigncode remove-signature -in "$exe" -out "$stripped"

# Read the four little-endian bytes of e_lfanew. The PE checksum is at
# e_lfanew + 0x58 for both PE32 and PE32+.
read -r b0 b1 b2 b3 < <(od -An -tu1 -j 60 -N 4 "$stripped")
checksum_offset=$((b0 + (b1 << 8) + (b2 << 16) + (b3 << 24) + 0x58))
printf '\0\0\0\0' | dd of="$stripped" bs=1 seek="$checksum_offset" conv=notrunc 2>/dev/null

if command -v sha256sum >/dev/null; then
    digest=$(sha256sum < "$stripped")
else
    digest=$(shasum -a 256 < "$stripped")
fi
hash=${digest%% *}
printf 'Normalized SHA-256: %s\n' "$hash"

if ! expected=$(awk -v name="$name" '
    substr($0, 67) == name && substr($0, 65, 2) ~ /^ [ *]$/ &&
    substr($0, 1, 64) !~ /[^0-9a-fA-F]/ && length($0) > 66 {
        count++
        hash = tolower(substr($0, 1, 64))
    }
    END { if (count != 1) exit 1; print hash }
' "$checksums"); then
    printf 'Expected one checksum for %s in %s.\n' "$name" "$checksums" >&2
    exit 1
fi
if [[ $hash != "$expected" ]]; then
    printf 'Checksum mismatch for %s.\n' "$name" >&2
    exit 1
fi

(set -o noclobber; cat "$stripped" > "$normalized")
printf 'Checksum matches. Verify the GitHub attestation with:\n'
printf 'gh attestation verify %q --repo brksfrb/arctic-launcher\n' "$normalized"
