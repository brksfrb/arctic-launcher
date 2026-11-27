"""Move raw display text out of the code and into the English catalogs.

Usage:  python tools/i18n_extract.py [--apply] [path-prefix ...]

Finds every literal tools/i18n_check.py would flag and, for the mechanical cases, rewrites it:

    ui.label("Show snapshots")            →  ui.label(t!("settings.show_snapshots"))
    format!("Installing {}", name)        →  tf!("tasks.installing", name = name)
    new Label("Waypoints", SECTION)       →  new Label(Lang.t("mods.waypoints"), SECTION)
    "Reconnect " + n                      →  Lang.t("platform.reconnect") + n

and writes the English text to `<catalog>/en/<area>.json` (the area follows the file's place
in the tree, see AREAS). Keys are the text's slug under its area; the same text in one area
gets one key. Cases that need a person (a `const`/`static final` table, a `match`/`case`
pattern, a `write!` with placeholders) are listed instead of changed. Without `--apply`
nothing is written; the planned changes and the manual list are printed.

After applying: handle the manual list, add `use crate::{t, tf, tn};` where the tool could not
(it tells you), run tools/i18n_check.py, and translate.
"""
from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import i18n_check as chk  # noqa: E402

ROOT = chk.ROOT

# (regex on the repo-relative path, area). First match wins.
RUST_AREAS = [
    (r"src/ui/(nav|shell|quick_tips|sponsor|about|suggest)\.rs$", "nav"),
    (r"src/(toasts|tray|tray_host|titlebar|window|zoom|widgets|start_menu|desktop_shortcut|main|fonts|glass)\.rs$", "nav"),
    (r"src/ui/(play|play_worlds|update_prompts|onboarding|dialogs|game_defaults|profiles|logs)\.rs$", "play"),
    (r"src/ui/(instances/|discover\.rs)", "instances"),
    (r"src/ui/(skins/|client_style\.rs)", "skins"),
    (r"src/ui/accounts\.rs$", "accounts"),
    (r"src/ui/(friends|together|chat|sharing|voice_settings|servers)\.rs$", "social"),
    (r"src/ui/(settings|network|storage|migrate|screenshots)\.rs$", "settings"),
    (r"src/.*\.rs$", "tasks"),
]
JAVA_AREAS = [
    (r"mod/versions/", "platform"),
    (r"client/menu/(FriendsTab|FriendsPage|ChatTab|ChatForms|AccountTab|AccountPage|SharePage)\.java$", "social"),
    (r"client/(account|social|together|tiers|voice|share)/", "social"),
    (r"client/menu/(CosmeticTiles|CapeTile|SkinTile|LooksCheckTab|PacksTab|PackRow)\.java$", "looks"),
    (r"client/(looks|packs)/", "looks"),
    (r"client/(servers|net)/", "servers"),
    (r"client/(replay/|menu/ReplaysTab\.java)", "replay"),
    (r"client/(hud|feature|waypoints|controller|config|items)/", "hud"),
    (r"client/", "mods"),
]

RUST_KEYWORDS = {
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "async", "await", "dyn", "n",
}

FORMAT_MACROS = ("format!(", "println!(", "eprintln!(", "print!(", "eprint!(", "write!(", "writeln!(")


def area_of(rel: str, java: bool) -> str:
    for pattern, area in JAVA_AREAS if java else RUST_AREAS:
        if re.search(pattern, rel):
            return area
    return "platform" if java else "tasks"


def unescape(raw: str, java: bool) -> str:
    """The string a Rust or Java literal's source stands for."""
    out = []
    i, n = 0, len(raw)
    while i < n:
        c = raw[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        i += 1
        if i >= n:
            break
        e = raw[i]
        if e == "n":
            out.append("\n")
        elif e == "t":
            out.append("\t")
        elif e == "r":
            out.append("\r")
        elif e == "0":
            out.append("\0")
        elif e in "\"'\\":
            out.append(e)
        elif e == "u":
            if java:
                out.append(chr(int(raw[i + 1 : i + 5], 16)))
                i += 4
            else:
                end = raw.index("}", i)
                out.append(chr(int(raw[i + 2 : end], 16)))
                i = end
        elif e == "\n" and not java:
            # Rust line continuation: the newline and the next line's leading spaces vanish.
            while i + 1 < n and raw[i + 1] in " \t":
                i += 1
        else:
            out.append("\\" + e)
        i += 1
    return "".join(out)


def escape_json_text(s: str) -> str:
    return s


def slug(text: str) -> str:
    base = chk.PLACEHOLDER_RE.sub(" ", text)
    base = re.sub(r"%[-0-9.]*[sdf]", " ", base)
    base = base.lower()
    base = re.sub(r"['’]", "", base)
    base = re.sub(r"[^a-z0-9]+", "_", base).strip("_")
    words = base.split("_")
    out = []
    for w in words:
        if sum(len(x) + 1 for x in out) + len(w) > 44 and out:
            break
        out.append(w)
    return "_".join(out) or "text"


class Catalog:
    def __init__(self, root: Path):
        self.root = root
        self.areas: dict[str, dict[str, str]] = {}
        self.by_text: dict[str, dict[str, str]] = defaultdict(dict)
        for file in sorted((root / "en").glob("*.json")) if (root / "en").is_dir() else []:
            data = json.loads(file.read_text(encoding="utf-8-sig"))
            self.areas[file.stem] = dict(data)
            for k, v in data.items():
                self.by_text[file.stem].setdefault(v, k)
        self.added = 0

    def key_for(self, area: str, text: str) -> str:
        strings = self.areas.setdefault(area, {})
        known = self.by_text[area].get(text)
        if known:
            return known
        base = f"{area}.{slug(text)}"
        key = base
        i = 2
        while key in strings:
            key = f"{base}_{i}"
            i += 1
        strings[key] = text
        self.by_text[area][text] = key
        self.added += 1
        return key

    def save(self):
        for area, strings in self.areas.items():
            path = self.root / "en" / f"{area}.json"
            path.parent.mkdir(parents=True, exist_ok=True)
            body = json.dumps(dict(sorted(strings.items())), ensure_ascii=False, indent=2)
            path.write_bytes((body + "\n").encode("utf-8"))


def matching_paren(src: str, open_pos: int) -> int:
    """Offset of the `)` that closes the `(` at open_pos (strings and chars skipped), or -1."""
    depth = 0
    i = open_pos
    n = len(src)
    while i < n:
        c = src[i]
        if c == '"':
            m = chk.RUST_STRING.match(src, i)
            if not m:
                return -1
            i = m.end()
            continue
        if c == "'":
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^'\\\n])'", src[i:])
            if m:
                i += len(m.group(0))
                continue
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def split_args(s: str) -> list[str]:
    """Top-level comma split of an argument list."""
    out, depth, start, i = [], 0, 0, 0
    n = len(s)
    while i < n:
        c = s[i]
        if c == '"':
            m = chk.RUST_STRING.match(s, i)
            i = m.end() if m else i + 1
            continue
        if c == "'":
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^'\\\n])'", s[i:])
            if m:
                i += len(m.group(0))
                continue
        if c in "([{<" and not (c == "<" and i > 0 and s[i - 1] in " =<>!-"):
            depth += 1
        elif c in ")]}>" and not (c == ">" and i > 0 and s[i - 1] in " =<>!-"):
            depth -= 1
        elif c == "," and depth == 0:
            out.append(s[start:i].strip())
            start = i + 1
        i += 1
    tail = s[start:].strip()
    if tail:
        out.append(tail)
    return out


def arg_name(expr: str, taken: set[str]) -> str:
    e = expr.strip()
    e = re.sub(r"^&+(mut\s+)?", "", e)
    name = None
    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", e):
        name = e
    else:
        m = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*\(\)\s*$", e)
        if m and m.group(1) in ("len", "count"):
            name = "count"
        elif m and m.group(1) in ("display", "to_string", "to_owned", "as_str", "clone", "trim", "unwrap", "unwrap_or_default"):
            before = e[: m.start()].rstrip(".")
            m2 = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*(\(\))?$", before)
            name = m2.group(1) if m2 else None
        else:
            m3 = re.search(r"([A-Za-z_][A-Za-z0-9_]*)\s*(\([^()]*\))?\s*$", e)
            name = m3.group(1) if m3 else None
    if not name or name in RUST_KEYWORDS or name.startswith("__"):
        name = "value"
    name = name.lower() if name.isupper() else name
    base, i = name, 2
    while name in taken:
        name = f"{base}{i}"
        i += 1
    taken.add(name)
    return name


FMT_PLACEHOLDER = re.compile(r"\{\{|\}\}|\{([A-Za-z_][A-Za-z0-9_]*|\d+)?(:[^}]*)?\}")


def rust_format(literal_text: str, args: list[str]):
    """(catalog text with {name} placeholders, tf! argument list) or None when too odd."""
    taken: set[str] = set()
    named: dict[str, str] = {}
    positional: list[str] = []
    for a in args:
        m = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)", a, re.DOTALL)
        if m and not a.startswith("="):
            named[m.group(1)] = m.group(2).strip()
        else:
            positional.append(a)
    out_args: list[tuple[str, str]] = []
    next_pos = 0
    text_out = []
    last = 0
    for m in FMT_PLACEHOLDER.finditer(literal_text):
        text_out.append(literal_text[last : m.start()])
        last = m.end()
        tok = m.group(0)
        if tok == "{{":
            text_out.append("{")
            continue
        if tok == "}}":
            text_out.append("}")
            continue
        ident, spec = m.group(1), m.group(2) or ""
        if ident is None:
            if next_pos >= len(positional):
                return None
            expr = positional[next_pos]
            next_pos += 1
            name = arg_name(expr, taken)
        elif ident.isdigit():
            idx = int(ident)
            if idx >= len(positional):
                return None
            expr = positional[idx]
            name = arg_name(expr, taken)
        else:
            name = ident
            expr = named.get(ident, ident)
            taken.add(name)
        if spec:
            inner = ident if (ident and not ident.isdigit() and ident not in named) else ""
            if inner:
                expr = f"format!(\"{{{inner}{spec}}}\")"
            else:
                expr = f"format!(\"{{{spec}}}\", {expr})"
        if name in RUST_KEYWORDS:
            name = f"{name}_value"
        out_args.append((name, expr))
        text_out.append("{" + name + "}")
    text_out.append(literal_text[last:])
    if next_pos != len(positional):
        return None
    return "".join(text_out), out_args


def rust_const_context(src: str, pos: int) -> bool:
    """Is the literal inside a `const`/`static` item (where a macro call isn't allowed)?"""
    line_start = src.rfind("\n", 0, pos) + 1
    i = line_start
    for _ in range(40):
        line_end = i - 1
        if line_end <= 0:
            return False
        i = src.rfind("\n", 0, line_end) + 1
        line = src[i:line_end]
        stripped = line.strip()
        if re.match(r"(pub(\([^)]*\))?\s+)?(const|static)\s", stripped):
            return True
        if re.match(r"(pub(\([^)]*\))?\s+)?(fn|impl|struct|enum|let|match|if|for|while|loop|mod|use|trait)\b", stripped) or stripped.endswith(";") or stripped in ("}", "};", "})", "});"):
            return False
    return False


def java_static_context(src: str, pos: int) -> bool:
    line_start = src.rfind("\n", 0, pos) + 1
    i = line_start
    first = True
    for _ in range(40):
        line_end = len(src) if first else i - 1
        if first:
            nl = src.find("\n", line_start)
            line_end = len(src) if nl < 0 else nl
        first = False
        line = src[i:line_end]
        stripped = line.strip()
        if re.search(r"\bstatic\b.*=", stripped) and not re.search(r"\b(return|if|for|while)\b", stripped):
            return True
        if stripped.endswith(";") or stripped.endswith("{") or stripped.endswith("}"):
            return False
        if i == 0:
            return False
        line_end = i - 1
        i = src.rfind("\n", 0, line_end) + 1
    return False


def after(src: str, pos: int) -> str:
    """Up to 12 chars after pos, whitespace skipped."""
    j = pos
    while j < len(src) and src[j] in " \t\n\r":
        j += 1
    return src[j : j + 12]


def before(src: str, pos: int) -> str:
    j = pos
    while j > 0 and src[j - 1] in " \t\n\r":
        j -= 1
    return src[max(0, j - 12) : j]


def plan_file(path: Path, java: bool, catalog: Catalog):
    rel = path.relative_to(ROOT).as_posix()
    raw = path.read_text(encoding="utf-8")
    head = "\n".join(raw.splitlines()[:5])
    if "i18n-ignore-file" in head:
        return raw, [], [], set()
    src = chk.strip_comments(raw)
    limit = len(src)
    if not java:
        cut = src.find("#[cfg(test)]")
        if cut >= 0:
            limit = cut
    area = area_of(rel, java)
    edits: list[tuple[int, int, str]] = []  # (start, end, replacement) on raw
    manual: list[str] = []
    macros: set[str] = set()
    lits = list(chk.literals(src, java))
    taken_ranges: list[tuple[int, int]] = []
    for pos, text in lits:
        shown = chk.java_drawn(src, pos, text) if java else chk.drawn_directly(src, pos, text)
        if pos >= limit or not (chk.looks_like_text(text) or shown):
            continue
        if any(a <= pos < b for a, b in taken_ranges):
            continue
        line_no = src.count("\n", 0, pos) + 1
        line_start = src.rfind("\n", 0, pos) + 1
        nl = src.find("\n", pos)
        line = src[line_start : nl if nl >= 0 else len(src)]
        if "i18n-ignore" in line:
            continue
        stripped = line.lstrip()
        if stripped.startswith("@") or stripped.startswith("#["):
            continue
        context = chk.enclosing_call(src, pos)
        if not (java and shown) and chk.EXEMPT_RE.search(context):
            continue
        # The literal's full source span.
        if java:
            m = chk.JAVA_STRING.match(src, pos)
        else:
            m = chk.RUST_STRING.match(src, pos)
        if not m:
            continue
        start, end = m.start(), m.end()
        if not java and m.group(1) is not None:
            manual.append(f"{rel}:{line_no}: raw string literal: {text[:60]!r}")
            continue
        nxt = after(src, end)
        prv = before(src, start)
        if nxt.startswith("=>") or (nxt.startswith("|") and not java) or nxt.startswith("==") or nxt.startswith("!=") or prv.endswith("==") or prv.endswith("!="):
            continue  # a pattern or comparison: an identity, not display text
        if java and (nxt.startswith(":") and prv.endswith("case")):
            continue
        if java and nxt.startswith(".equals"):
            continue
        if not java and re.search(r"\bconst\s|\bstatic\s", line) or (not java and rust_const_context(src, pos)):
            manual.append(f"{rel}:{line_no}: const/static table: {text[:60]!r}")
            continue
        if java and java_static_context(src, pos):
            manual.append(f"{rel}:{line_no}: static field: {text[:60]!r}")
            continue
        value = unescape(text, java)
        if java:
            key = catalog.key_for(area, value)
            edits.append((start, end, f'Lang.t("{key}")'))
            continue
        # Rust: a format macro's format string?
        ctx_tail = context.rstrip()
        macro = next((mac for mac in FORMAT_MACROS if ctx_tail.endswith(mac)), None)
        if macro is None and re.search(r"\b(write|writeln)!\(\s*[A-Za-z_][A-Za-z0-9_.]*\s*,\s*$", context):
            macro = "write!("
        if macro:
            open_pos = context.rfind("(")
            # Position of that "(" in src:
            macro_open = start - (len(context) - open_pos) if ctx_tail == context else None
            if macro_open is None:
                macro_open = src.rfind("(", 0, start)
            close = matching_paren(src, macro_open)
            if close < 0:
                manual.append(f"{rel}:{line_no}: format string (unparsed): {text[:60]!r}")
                continue
            rest = src[end:close]
            args = split_args(rest.lstrip(",").strip()) if rest.strip().startswith(",") else ([] if not rest.strip() else None)
            if args is None:
                manual.append(f"{rel}:{line_no}: format string (odd args): {text[:60]!r}")
                continue
            parsed = rust_format(value, args)
            if parsed is None:
                manual.append(f"{rel}:{line_no}: format string (placeholders vs args): {text[:60]!r}")
                continue
            cat_text, tf_args = parsed
            if macro == "format!(":
                key = catalog.key_for(area, cat_text)
                call = "tf!(" + json.dumps(key) + "".join(f", {n} = {e}" for n, e in tf_args) + ")"
                # Replace the whole format!(...) call.
                mstart = src.rfind("format!", 0, start)
                edits.append((mstart, close + 1, call))
                taken_ranges.append((mstart, close + 1))
                macros.add("tf")
            else:
                if tf_args:
                    manual.append(f"{rel}:{line_no}: {macro} with placeholders: {text[:60]!r}")
                    continue
                key = catalog.key_for(area, cat_text)
                edits.append((start, end, f'"{{}}", t!({json.dumps(key)})'))
                macros.add("t")
            continue
        if "{" in value and re.search(r"\{[A-Za-z_]\w*\}", value):
            manual.append(f"{rel}:{line_no}: has placeholders outside format!: {text[:60]!r}")
            continue
        key = catalog.key_for(area, value)
        edits.append((start, end, f"t!({json.dumps(key)})"))
        macros.add("t")
    return raw, edits, manual, macros


def apply_edits(raw: str, edits: list[tuple[int, int, str]]) -> str:
    out = raw
    for start, end, rep in sorted(edits, key=lambda e: e[0], reverse=True):
        out = out[:start] + rep + out[end:]
    return out


def add_rust_import(text: str, macros: set[str]) -> tuple[str, bool]:
    if not macros:
        return text, True
    wanted = sorted(macros)
    existing = re.search(r"^use crate::\{([^}]*)\};\s*$", text, re.M)
    if existing and all(m in [x.strip() for x in existing.group(1).split(",")] for m in wanted):
        return text, True
    if existing:
        names = sorted({x.strip() for x in existing.group(1).split(",") if x.strip()} | set(wanted))
        if all(n in ("t", "tf", "tn") for n in names):
            return text[: existing.start()] + "use crate::{" + ", ".join(names) + "};" + text[existing.end() :], True
    line = "use crate::{" + ", ".join(wanted) + "};"
    # After the last top-level `use …;` (single or multi-line) before the first item.
    uses = list(re.finditer(r"^use [^;]*;\s*$", text, re.M | re.S))
    if uses:
        last = uses[-1]
        return text[: last.end()].rstrip("\n") + "\n" + line + "\n" + text[last.end() :].lstrip("\n") if text[last.end():].startswith("\n\n") else text[: last.end()].rstrip("\n") + "\n" + line + text[last.end():], True
    # No imports: after the leading doc comments / attributes.
    m = re.match(r"((?://[^\n]*\n|#!\[[^\n]*\n|\s*\n)*)", text)
    head = m.group(0) if m else ""
    return head + line + "\n\n" + text[len(head) :], True


def add_java_import(text: str) -> str:
    if re.search(r"^package com\.arcticlauncher\.client;\s*$", text, re.M):
        return text
    if re.search(r"^import com\.arcticlauncher\.client\.Lang;", text, re.M):
        return text
    imports = list(re.finditer(r"^import [^\n]*;\s*$", text, re.M))
    if imports:
        last = imports[-1]
        return text[: last.end()] + "\nimport com.arcticlauncher.client.Lang;" + text[last.end() :]
    pkg = re.search(r"^package [^\n]*;\s*$", text, re.M)
    if pkg:
        return text[: pkg.end()] + "\n\nimport com.arcticlauncher.client.Lang;" + text[pkg.end() :]
    return "import com.arcticlauncher.client.Lang;\n" + text


def main() -> int:
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")
    apply = "--apply" in sys.argv
    prefixes = [a for a in sys.argv[1:] if not a.startswith("--")]
    total_edits = 0
    manual_all: list[str] = []
    for name, roots, catalog_root, suffix in chk.SIDES:
        java = suffix == ".java"
        catalog = Catalog(catalog_root)
        files = []
        for root in roots:
            files += sorted(p for p in root.rglob(f"*{suffix}") if not chk.SKIP_FILES.search(str(p)))
        for path in files:
            rel = path.relative_to(ROOT).as_posix()
            if prefixes and not any(rel.startswith(p) for p in prefixes):
                continue
            raw, edits, manual, macros = plan_file(path, java, catalog)
            manual_all += manual
            if not edits:
                continue
            total_edits += len(edits)
            new = apply_edits(raw, edits)
            if java:
                new = add_java_import(new)
            else:
                new, _ = add_rust_import(new, macros)
            if apply:
                path.write_bytes(new.encode("utf-8"))
            print(f"{rel}: {len(edits)} replacement(s)" + (f", macros {sorted(macros)}" if macros else ""))
        if apply:
            catalog.save()
        print(f"{name}: {catalog.added} key(s) added")
    print(f"\n{total_edits} replacement(s) {'applied' if apply else 'planned'}; {len(manual_all)} for a person:")
    for m in manual_all:
        print("  " + m)
    return 0


if __name__ == "__main__":
    sys.exit(main())
