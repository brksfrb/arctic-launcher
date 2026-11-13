"""Put a wrongly extracted key's English text back into the code as a literal.

Usage:  python tools/i18n_revert.py key [key ...]

For each key: every `t!("key")` / `tf!("key", a = x)` / `Lang.t("key")` in the sources becomes
the literal again (a `format!` for `tf!`), the line gets `// i18n-ignore`, and the key leaves
the English catalog. For text the extractor took that people never see (commands, shader
source, log prefixes, search keywords).
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import i18n_check as chk  # noqa: E402


def literal(text: str) -> str:
    return json.dumps(text, ensure_ascii=False)


def main() -> int:
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")
    keys = [a for a in sys.argv[1:] if not a.startswith("--")]
    for name, roots, catalog_root, suffix in chk.SIDES:
        java = suffix == ".java"
        files = []
        for root in roots:
            files += sorted(p for p in root.rglob(f"*{suffix}") if not chk.SKIP_FILES.search(str(p)))
        for file in sorted((catalog_root / "en").glob("*.json")):
            data = json.loads(file.read_bytes().decode("utf-8-sig"))
            hit = [k for k in keys if k in data]
            if not hit:
                continue
            for key in hit:
                text = data.pop(key)
                lit = literal(text)
                count = 0
                for path in files:
                    src = path.read_bytes().decode("utf-8")
                    if key not in src:
                        continue
                    if java:
                        pattern = re.compile(r"Lang\.t\(" + re.escape(json.dumps(key)) + r"\)")
                        new, n = pattern.subn(lambda m: lit, src)
                    else:
                        pattern = re.compile(r"\bt!\(" + re.escape(json.dumps(key)) + r"\)")
                        new, n = pattern.subn(lambda m: lit, src)
                        tf = re.compile(r"\btf!\(" + re.escape(json.dumps(key)) + r"((?:,\s*\w+\s*=\s*[^,()]+(?:\([^()]*\))?)*)\s*\)")
                        new, n2 = tf.subn(lambda m: "format!(" + lit + m.group(1) + ")", new)
                        n += n2
                    if n:
                        # Mark the lines so the checker leaves them alone.
                        lines = new.split("\n")
                        for i, line in enumerate(lines):
                            if lit in line and "i18n-ignore" not in line:
                                lines[i] = line.rstrip() + " // i18n-ignore"
                        path.write_bytes("\n".join(lines).encode("utf-8"))
                        count += n
                print(f"{key}: {count} place(s) reverted to {lit[:60]}")
            file.write_bytes((json.dumps(dict(sorted(data.items())), ensure_ascii=False, indent=2) + "\n").encode("utf-8"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
