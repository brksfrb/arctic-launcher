"""Check that every piece of text the launcher and the Arctic Client show is localized.

Usage (from anywhere):  python tools/i18n_check.py [--verbose] [--unused]

What it checks, and fails on (exit code 1):

1. **No raw text in the code.** Every string literal in the launcher's UI crate
   (crates/arctic-app) and the mod (mod/core, mod/versions/*) that reads like text people see
   (a capitalised word, a sentence, something ending in . ? ! or …) must be a translation call:
   `t!("key")`, `tf!("key", name = v)`, `tn!("key", n)` in Rust, `Lang.t("key")`,
   `Lang.plural("key", n)` in Java. Literals in logging, assertions, ids, paths, keys, URLs,
   headers, comparisons and the like are left alone (see EXEMPT_CALLS and looks_like_text). A line
   that must keep a literal says so with `// i18n-ignore` (or `/* i18n-ignore */` before it);
   a whole file with `// i18n-ignore-file` in its first lines. tools/i18n_allow.txt lists
   brand and technical words that are never translated (one regex per line, matched whole).

2. **Every key used exists in English.** `crates/arctic-app/locales/en/*.json` and
   `mod/core/src/main/resources/assets/arctic/lang/en/*.json`; a plural key needs `.other`.

3. **Every language is complete.** Each language has every English key (no more, no less),
   with the same `{placeholders}`, no empty text, no key repeated across its files, and every
   file is a flat JSON object of strings.

`--unused` also lists English keys no code uses (a warning: some keys are built at run time).
"""
from __future__ import annotations

import json
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ALLOW_FILE = ROOT / "tools" / "i18n_allow.txt"

# (name, source roots, catalog root, file suffix)
SIDES = [
    ("launcher", [ROOT / "crates" / "arctic-app" / "src"], ROOT / "crates" / "arctic-app" / "locales", ".rs"),
    (
        "mod",
        [ROOT / "mod" / "core" / "src" / "main" / "java"]
        + sorted((ROOT / "mod" / "versions").glob("*/src/main/java")),
        ROOT / "mod" / "core" / "src" / "main" / "resources" / "assets" / "arctic" / "lang",
        ".java",
    ),
]

# Files that are never text people see (drawing, tests, tooling, the framework itself).
SKIP_FILES = re.compile(
    r"(^|[\\/])(i18n\.rs|icon_raster\.rs|memstats\.rs|devshot\.rs|video_encoder\.rs|"
    r"video_encoder_stream\.rs|video_decoder\.rs|Lang\.java|Smoke\.java|"
    r"[A-Za-z]*Test[A-Za-z]*\.java|SelfTest\.java|PromoTest\.java|WorldTest\.java)$"
    r"|[\\/]art[\\/]|[\\/]test[\\/]|[\\/]build[\\/]"
)

# A literal that is an argument of one of these (anywhere earlier on the same line, or the
# nearest open call) is not text people see.
EXEMPT_CALLS = [
    # Rust
    r"\bt!\(", r"\btf!\(", r"\btn!\(", r"i18n::(text|lookup|format|plural|core_text|supported|set_language)\(",
    r"\blog::\w+!\(", r"\b(debug|info|warn|error|trace)!\(", r"\bpanic!\(", r"\bassert(_eq|_ne)?!\(",
    r"\bdebug_assert(_eq|_ne)?!\(", r"\bunreachable!\(", r"\btodo!\(", r"\bunimplemented!\(",
    r"\.expect\(", r"\.expect_err\(", r"Id::new\(", r"\.id_salt\(", r"\.with\(", r"\benv!\(", r"\boption_env!\(",
    r"\binclude_str!\(", r"\binclude_bytes!\(", r"\bcfg!\(", r"\bconcat!\(", r"\bmatches!\(",
    r"Command::new\(", r"\.args?\(", r"\.env\(", r"\.header\(", r"\.set_header\(", r"\.query\(",
    r"\.starts_with\(", r"\.ends_with\(", r"\.contains\(", r"\.strip_prefix\(", r"\.strip_suffix\(",
    r"\.split\w*\(", r"\.replace\w*\(", r"\.find\(", r"\.rfind\(", r"\.trim\w*\(", r"\.eq_ignore_ascii_case\(",
    r"\.parse\(", r"\.extension\(\)\s*==", r"Path::new\(", r"PathBuf::from\(", r"\.join\(",
    r"\.set_file_name\(", r"\.set_extension\(", r"\.with_extension\(", r"\.with_file_name\(",
    r"\.open::<", r"\.get\(", r"\.insert\(", r"\.remove\(", r"\.entry\(", r"\.key\(",
    r"\bformat_args!\(",
    r"serde_json::json!\(", r"#\[", r"\.with_app_id\(", r"\.with_title\(",
    # Java
    r"\bLang\.(t|plural|lookup|use|supported)\(", r"\bLOG\.\w+\(", r"\.log\(", r"\blog\(",
    r"\.(warn|info|error|debug|trace|severe|fine)\(", r"System\.(getProperty|getenv|setProperty)\(",
    r"getResource(AsStream)?\(", r"new File\(", r"Paths\.get\(", r"\.resolve\(", r"\.equals(IgnoreCase)?\(",
    r"\.startsWith\(", r"\.endsWith\(", r"\.contains\(", r"\.indexOf\(", r"\.lastIndexOf\(", r"\.split\(",
    r"\.replace(All|First)?\(", r"\.matches\(", r"Pattern\.compile\(", r"\.compareTo\(",
    r"\bIdentifier(\.of)?\(", r"new Identifier\(", r"ResourceLocation(\.(fromNamespaceAndPath|withDefaultNamespace|parse|tryParse))?\(",
    r"new ResourceLocation\(", r"\btranslate\(", r"\.translatable\(", r"Component\.translatable\(",
    r"Text\.translatable\(", r"new TranslatableText\(", r"\bthrow new \w+\(", r"\bnew \w*Exception\(",
    r"\bassert\b", r"Class\.forName\(", r"\.getMethod\(", r"\.getDeclaredMethod\(", r"\.getField\(",
    r"\.getDeclaredField\(", r"\.getDeclaredConstructor\(", r"\.setProperty\(", r"\.getProperty\(",
    r"\.header\(", r"\.setRequestProperty\(", r"\.addRequestProperty\(", r"new URL\(", r"URI\.create\(",
    r"\.openConnection\(", r"\.put\(", r"\.get\(", r"\.remove\(", r"\.containsKey\(", r"\.getAsJsonObject\(",
    r"\.getAsJsonArray\(", r"\.has\(", r"\.add\(\s*\"", r"\.addProperty\(", r"\.keyName\(", r"\.keyLabel\(",
    r"\.isKeyDown\(", r"\.physicalKeyDown\(", r"GameKey\.\w+\(", r"\.supportsGameWidget\(", r"\.openPage\(\s*\"",
    r"Menus\.(open|page)\(", r"\.ofNullable\(", r"String\.format\(\s*(java\.util\.)?Locale\.\w+,\s*\"[^\"]*%",
    r"\bnew Thread\(", r"\.setName\(", r"@\w+\(", r"\bLegacyPlatform\.LOG", r"\bArcticMod\.LOG", r"\bArcticLegacy\.LOG",
]
EXEMPT_RE = re.compile("|".join(f"(?:{p})" for p in EXEMPT_CALLS))

# Rust `tn!`/Java `plural` need `.other`; everything else the key itself.
RUST_KEY_CALLS = re.compile(r'\b(t!|tf!|tn!|i18n::text|i18n::format|i18n::plural)\(\s*"((?:[^"\\]|\\.)*)"')
JAVA_KEY_CALLS = re.compile(r'\bLang\.(t|plural)\(\s*"((?:[^"\\]|\\.)*)"')
PLURAL_CALLS = {"tn!", "i18n::plural", "plural"}

PLACEHOLDER_RE = re.compile(r"\{([^{}]*)\}")
LETTER_RE = re.compile(r"[^\W\d_]", re.UNICODE)
UPPER_START = re.compile(r"^[\"'(\[]?[A-ZÀ-ÞĀ-ŽА-ЯЁІЇЄҐ]")
WORDS_WITH_SPACE = re.compile(r"[^\W\d_]{2,}\s+[^\W\d_]{2,}", re.UNICODE)
SENTENCE_END = re.compile(r"[^\W\d_][.?!…:]\s*$", re.UNICODE)
NOT_TEXT = [
    re.compile(r"^(?=[A-Z0-9_]*[0-9_])[A-Z0-9_]+$|^[A-Z]{1,2}$"),  # CONSTANTS_WITH_UNDERSCORES, X1
    re.compile(r"^[a-z0-9_.:/\\-]+$"),  # ids, keys, paths
    re.compile(r"^[a-z]+([A-Z][a-z0-9]*)+$"),  # camelCase identifiers
    re.compile(r"^[A-Z][a-z]+(-[A-Z][a-z]+)+$"),  # Content-Type
    re.compile(r"://"),  # URLs
    re.compile(r"^[\s\W]*$"),  # punctuation only
    re.compile(r"^#[0-9A-Fa-f]{3,8}$"),  # colors
    re.compile(r"^[A-Za-z]:\\"),  # Windows paths
    re.compile(r"^\.?[A-Za-z0-9_-]+\.[a-z0-9]{1,5}$"),  # file names
    re.compile(r"^\{\w*\}$"),  # "{}" / "{name}" alone
    re.compile(r"^(key\.|minecraft:|arctic:|assets/|textures/|com\.|net\.|java\.|org\.)"),
    re.compile(r"^%[-0-9.]*[sdf]$"),  # printf specs
    re.compile(r"^[A-Z][a-z]?$"),  # a one- or two-letter label like "X", "Ab"
    re.compile(r"^\S*[/;\\]\S*$"),  # JVM descriptors, paths, "a/b"
    re.compile(r"^[yMdHhmsSaEz:.,\-/' ]+$"),  # date and time formats ("HH:mm", "MM-dd HH.mm")
    re.compile(r"^[a-z0-9]+( [a-z0-9]+){2,}$"),  # search keyword lists ("hud widget ping latency")
    re.compile(r"(?=.*\\n)(?=.*[;{}=])"),  # shader and other source code with line breaks
    re.compile(r"^[\[(][A-Za-z;/\[\]()]*$"),  # "[B", "(Ljava/lang/String;)V"
]


PLURAL_FORMS = {"zero", "one", "two", "few", "many", "other"}


def plural_form_of(key: str, english: dict) -> str | None:
    """English's `.other` text when `key` is a plural form a language adds ("x.few")."""
    base, _, form = key.rpartition(".")
    if form in PLURAL_FORMS:
        return english.get(f"{base}.other")
    return None


def load_allow() -> list[re.Pattern[str]]:
    if not ALLOW_FILE.is_file():
        return []
    out = []
    for line in ALLOW_FILE.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            out.append(re.compile(f"^(?:{line})$"))
    return out


ALLOW = load_allow()


def looks_like_text(s: str) -> bool:
    """Does this literal read like words people see?"""
    s = s.strip()
    if not LETTER_RE.search(s):
        return False
    if any(p.search(s) for p in NOT_TEXT):
        return False
    if any(p.match(s) for p in ALLOW):
        return False
    # A format string that is only placeholders and punctuation ("{} × {}", "{}: {}").
    if not LETTER_RE.search(PLACEHOLDER_RE.sub("", s).replace("%s", "").replace("%d", "")):
        return False
    if re.fullmatch(r"[A-Z]{3,}( [A-Z]{2,})*", s):
        return True  # an all-caps heading or button ("PLAY", "WORLDS")
    # A fragment led by a placeholder: "{} instance", "{n} mods".
    if PLACEHOLDER_RE.match(s) and re.search(r"\s[^\W\d_]{3,}", PLACEHOLDER_RE.sub("", s)):
        return True
    return bool(UPPER_START.match(s) or WORDS_WITH_SPACE.search(s) or SENTENCE_END.search(s))


def strip_comments(src: str) -> str:
    """Comments and char literals blanked (same length, so offsets and lines hold)."""
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            j = n if j < 0 else j
            # Keep i18n-ignore markers readable for the line check.
            comment = src[i:j]
            out.append(comment if "i18n-ignore" in comment else " " * (j - i))
            i = j
        elif src.startswith("/*", i):
            j = src.find("*/", i + 2)
            j = n if j < 0 else j + 2
            comment = src[i:j]
            out.append(re.sub(r"[^\n]", " ", comment) if "i18n-ignore" not in comment else comment)
            i = j
        elif c == '"' or (c == "r" and re.match(r'r#*"', src[i : i + 8])):
            # String literal: copy verbatim.
            if c == "r":
                m = re.match(r'r(#*)"', src[i:])
                hashes = m.group(1)
                end = src.find('"' + hashes, i + len(m.group(0)))
                end = n if end < 0 else end + 1 + len(hashes)
                out.append(src[i:end])
                i = end
            else:
                j = i + 1
                while j < n and src[j] != '"':
                    j += 2 if src[j] == "\\" else 1
                out.append(src[i : min(j + 1, n)])
                i = j + 1
        elif c == "'":
            # A char literal ('a', '\n', '\u{1F}') or a lifetime: blank chars, keep lifetimes.
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^'\\\n])'", src[i:])
            if m:
                out.append(" " * len(m.group(0)))
                i += len(m.group(0))
            else:
                out.append(c)
                i += 1
        else:
            out.append(c)
            i += 1
    return "".join(out)


RUST_STRING = re.compile(r'r(#*)"(.*?)"\1|"((?:[^"\\\n]|\\.|\\\n)*)"', re.DOTALL)
JAVA_STRING = re.compile(r'"((?:[^"\\\n]|\\.)*)"')


def literals(src: str, java: bool):
    """(start offset, text) of every string literal."""
    pattern = JAVA_STRING if java else RUST_STRING
    for m in pattern.finditer(src):
        text = m.group(1) if java else (m.group(2) if m.group(2) is not None else m.group(3))
        yield m.start(), text


# A Rust literal handed straight to something that draws it is text whatever it looks like
# ("as {}", "online"), even lowercase or a single word.
DISPLAY_CALL = re.compile(
    r"(?:RichText::new|\.label|\.button|\.small_button|\.heading|\.hint_text|\.selected_text|"
    r"\.on_hover_text|\.selectable_label|\.checkbox|\.radio_value|\.radio|Button::new|Label::new|"
    r"\.monospace|\.strong|\.weak|\.code)\(\s*(?:&?\s*format!\(\s*)?$"
)


def drawn_directly(src: str, pos: int, text: str) -> bool:
    if not LETTER_RE.search(PLACEHOLDER_RE.sub("", text)):
        return False
    if any(p.search(text.strip()) for p in NOT_TEXT[1:]) or any(p.match(text.strip()) for p in ALLOW):
        return False
    return bool(DISPLAY_CALL.search(src[max(0, pos - 200) : pos]))


def enclosing_call(src: str, pos: int) -> str:
    """The code from the start of the innermost unclosed call (or the line) up to `pos`."""
    depth = 0
    i = pos - 1
    line_start = src.rfind("\n", 0, pos) + 1
    limit = max(0, pos - 1500)
    while i >= limit:
        c = src[i]
        if c in ")]}":
            depth += 1
        elif c in "([{":
            if depth == 0:
                # From the start of the line the call opens on (so `throw new X(`, `new Thread(`
                # and the callee's name are all in view).
                return src[src.rfind("\n", 0, i) + 1 : pos]
            depth -= 1
        elif c == ";" and depth == 0:
            break
        i -= 1
    return src[line_start:pos]


def check_sources(name: str, roots: list[Path], suffix: str, verbose: bool):
    """(raw text problems, keys used as {key: [locations]}, plural keys)."""
    java = suffix == ".java"
    problems = []
    used: dict[str, list[str]] = defaultdict(list)
    plural: set[str] = set()
    files = []
    for root in roots:
        files += sorted(p for p in root.rglob(f"*{suffix}") if not SKIP_FILES.search(str(p)))
    for path in files:
        raw = path.read_text(encoding="utf-8", errors="replace")
        head = "\n".join(raw.splitlines()[:5])
        if "i18n-ignore-file" in head:
            continue
        src = strip_comments(raw)
        # Rust tests at the end of a file aren't text people see.
        if not java:
            cut = src.find("#[cfg(test)]")
            if cut >= 0:
                src = src[:cut]
        rel = path.relative_to(ROOT).as_posix()
        key_calls = JAVA_KEY_CALLS if java else RUST_KEY_CALLS
        for m in key_calls.finditer(src):
            line = src.count("\n", 0, m.start()) + 1
            used[m.group(2)].append(f"{rel}:{line}")
            if m.group(1) in PLURAL_CALLS:
                plural.add(m.group(2))
        for pos, text in literals(src, java):
            if not looks_like_text(text) and (java or not drawn_directly(src, pos, text)):
                continue
            line_no = src.count("\n", 0, pos) + 1
            line_start = src.rfind("\n", 0, pos) + 1
            line_end = src.find("\n", pos)
            line = src[line_start : line_end if line_end >= 0 else len(src)]
            if "i18n-ignore" in line:
                continue
            context = enclosing_call(src, pos)
            if EXEMPT_RE.search(context):
                continue
            # Java annotations (mixin targets) and Rust attributes span lines.
            stripped = line.lstrip()
            if stripped.startswith("@") or stripped.startswith("#["):
                continue
            problems.append(f"{rel}:{line_no}: raw text {json.dumps(text, ensure_ascii=False)}")
    if verbose:
        print(f"{name}: {len(files)} source files, {len(used)} keys used")
    return problems, used, plural


def load_catalogs(name: str, root: Path):
    """{language: {key: text}}, with problems for broken files."""
    problems = []
    catalogs: dict[str, dict[str, str]] = {}
    sources: dict[str, dict[str, str]] = {}
    if not root.is_dir():
        return catalogs, [f"{name}: no catalogs at {root.relative_to(ROOT).as_posix()}"]
    for lang_dir in sorted(p for p in root.iterdir() if p.is_dir()):
        lang = lang_dir.name
        strings: dict[str, str] = {}
        where: dict[str, str] = {}
        for file in sorted(lang_dir.glob("*.json")):
            rel = file.relative_to(ROOT).as_posix()
            try:
                data = json.loads(file.read_text(encoding="utf-8-sig"))
            except ValueError as e:
                problems.append(f"{rel}: not valid JSON ({e})")
                continue
            if not isinstance(data, dict):
                problems.append(f"{rel}: must be a flat JSON object")
                continue
            for key, value in data.items():
                if not isinstance(value, str):
                    problems.append(f"{rel}: {key} is not a string")
                    continue
                if key in strings:
                    problems.append(f"{rel}: {key} already in {where[key]}")
                    continue
                if not value.strip():
                    problems.append(f"{rel}: {key} is empty")
                strings[key] = value
                where[key] = rel
        catalogs[lang] = strings
        sources[lang] = where
    return catalogs, problems


def placeholders(s: str) -> list[str]:
    return sorted(PLACEHOLDER_RE.findall(s))


KEY_LITERAL = re.compile(r'"([a-z0-9_]+(?:\.[a-z0-9_]+)+)"')


def check_side(name, roots, catalog_root, suffix, verbose, unused):
    errors = []
    warnings = []
    raw, used, plural = check_sources(name, roots, suffix, verbose)
    errors += raw
    catalogs, broken = load_catalogs(name, catalog_root)
    errors += broken
    english = catalogs.get("en", {})
    if not english:
        errors.append(f"{name}: no English catalog")
    for key, where in sorted(used.items()):
        if key in plural:
            if f"{key}.other" not in english and key not in english:
                errors.append(f"{name}: plural key {key} needs {key}.other in English (used at {where[0]})")
        elif key not in english:
            errors.append(f"{name}: key {key} is not in English (used at {where[0]})")
    for lang, strings in sorted(catalogs.items()):
        if lang == "en":
            continue
        missing = sorted(k for k in english if k not in strings)
        extra = sorted(k for k in strings if k not in english and not plural_form_of(k, english))
        if missing:
            shown = ", ".join(missing[:8]) + (", …" if len(missing) > 8 else "")
            errors.append(f"{name}/{lang}: {len(missing)} keys missing: {shown}")
        if extra:
            shown = ", ".join(extra[:8]) + (", …" if len(extra) > 8 else "")
            errors.append(f"{name}/{lang}: {len(extra)} keys not in English: {shown}")
        same = 0
        for key, value in strings.items():
            source = english.get(key) or plural_form_of(key, english)
            if source is None:
                continue
            ours, theirs = placeholders(source), placeholders(value)
            if key.rsplit(".", 1)[-1] in PLURAL_FORMS:
                ours, theirs = [x for x in ours if x != "n"], [x for x in theirs if x != "n"]
            if ours != theirs:
                errors.append(f"{name}/{lang}: {key} has placeholders {placeholders(value)}, English {placeholders(source)}")
            if value == source and looks_like_text(source) and len(source) > 3:
                same += 1
        if verbose and same:
            warnings.append(f"{name}/{lang}: {same} texts identical to English")
    if unused:
        plural_bases = {k.rsplit(".", 1)[0] for k in english if k.rsplit(".", 1)[-1] in ("one", "few", "many", "other", "zero")}
        # Tables of choices keep their keys as plain strings and translate them where shown.
        quoted = set()
        for root in roots:
            for path in root.rglob(f"*{suffix}"):
                if not SKIP_FILES.search(str(path)):
                    quoted.update(KEY_LITERAL.findall(path.read_text(encoding="utf-8", errors="replace")))
        for key in sorted(english):
            base = key.rsplit(".", 1)[0]
            if key not in used and key not in quoted and base not in used and base not in plural_bases and not key.startswith("core."):
                warnings.append(f"{name}: unused key {key}")
    if verbose:
        langs = ", ".join(f"{l}={len(s)}" for l, s in sorted(catalogs.items()))
        print(f"{name}: catalogs {langs}")
    return errors, warnings


def main() -> int:
    # Windows consoles default to a legacy code page; the texts are UTF-8.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")
    verbose = "--verbose" in sys.argv
    unused = "--unused" in sys.argv
    errors = []
    warnings = []
    for name, roots, catalog_root, suffix in SIDES:
        e, w = check_side(name, roots, catalog_root, suffix, verbose, unused)
        errors += e
        warnings += w
    for w in warnings:
        print(f"warning: {w}")
    for e in errors:
        print(f"error: {e}")
    if errors:
        print(f"\n{len(errors)} problem(s). Every text people see goes through t!/tf!/tn! (launcher) or "
              "Lang.t/Lang.plural (mod); see docs/i18n.md.")
        return 1
    print("i18n OK: all text localized, all languages complete")
    return 0


if __name__ == "__main__":
    sys.exit(main())
