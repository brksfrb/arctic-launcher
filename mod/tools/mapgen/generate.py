"""Make Mojang-named mappings for a Minecraft version Mojang never published names for (1.13.2).

Usage: python generate.py <work dir> <out.tiny>      (needs Python 3.8+, internet for the first run)

Mojang's official names exist from 1.14.4. This lines up 1.14.4's classes and members with the older
version's (they are close: both are the GLFW-era game) and writes a Tiny v2 file, intermediary ->
named, for Legacy Fabric's 1.13.2 intermediary. Gradle (Loom) then compiles the Fabric adapter's
Mojang-named sources against 1.13.2. What the matcher can't pair stays with its intermediary name;
`seeds-1.13.2.txt` pairs the classes it can't find by itself.
"""
import json
import os
import re
import sys
import urllib.request
import zipfile
from collections import Counter, defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import classfile  # noqa: E402
import match  # noqa: E402

OLD = "1.13.2"
NEW = "1.14.4"
INTERMEDIARY = "https://maven.legacyfabric.net/net/legacyfabric/intermediary/{v}/intermediary-{v}-v2.jar"
LF_YARN = "https://maven.legacyfabric.net/net/legacyfabric/yarn/{v}+build.604/yarn-{v}+build.604-v2.jar"
MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"


def fetch(url, path):
    if not os.path.exists(path):
        print("download", url)
        req = urllib.request.Request(url, headers={"User-Agent": "arctic-mapgen"})
        with urllib.request.urlopen(req) as r, open(path, "wb") as f:
            f.write(r.read())
    return path


def game_download(work, version, key, suffix):
    path = os.path.join(work, f"{version}-{key}{suffix}")
    if os.path.exists(path):
        return path
    manifest = json.load(urllib.request.urlopen(MANIFEST))
    meta_url = [v["url"] for v in manifest["versions"] if v["id"] == version][0]
    meta = json.load(urllib.request.urlopen(meta_url))
    return fetch(meta["downloads"][key]["url"], path)


def read_tiny(path, member):
    with zipfile.ZipFile(path) as z:
        return z.read(member).decode("utf-8").splitlines()


def prim(t):
    return {"int": "I", "long": "J", "float": "F", "double": "D", "boolean": "Z", "byte": "B", "short": "S", "char": "C", "void": "V"}.get(t)


def to_desc(t, cls_to_obf):
    dims = 0
    while t.endswith("[]"):
        dims += 1
        t = t[:-2]
    p = prim(t)
    base = p if p else "L" + cls_to_obf.get(t, t).replace(".", "/") + ";"
    return "[" * dims + base


def read_mojmap(path):
    """class mojmap->obf, and {(obf class, obf name, obf desc or 'F'): mojmap name}."""
    cls = {}
    raw = []
    cur = None
    for line in open(path, encoding="utf-8"):
        if line.startswith("#"):
            continue
        if not line.startswith(" "):
            m = re.match(r"(\S+) -> (\S+):", line)
            cur = (m.group(1), m.group(2))
            cls[cur[0]] = cur[1]
            continue
        m = re.match(r"\s+(?:\d+:\d+:)?(\S+) (\S+?)(?:\((.*?)\))?(?::\d+:\d+)? -> (\S+)$", line)
        if m:
            raw.append((cur[1], m.group(1), m.group(2), m.group(3), m.group(4)))
    names = {}
    for obf_cls, ty, name, params, obf in raw:
        oc = obf_cls.replace(".", "/")
        if params is None:
            names[(oc, obf, "F")] = name
        else:
            ps = "".join(to_desc(p, cls) for p in params.split(",")) if params else ""
            names[(oc, obf, "(" + ps + ")" + to_desc(ty, cls))] = name
    return cls, names


def yarn_members(yarn_jar):
    """Yarn: {(yarn class, yarn name): [(intermediary name, intermediary desc)]}, {yarn class: intermediary class}."""
    names = defaultdict(list)
    classes = {}
    cur = None
    cur_y = None
    for line in read_tiny(yarn_jar, "mappings/mappings.tiny"):
        p = line.split("	")
        if p[0] == "c":
            cur = p[1]
            cur_y = p[2]
            classes[p[2]] = p[1]
        elif p[0] == "" and len(p) >= 5 and p[1] in ("f", "m"):
            names[(cur_y, p[4])].append((p[3], p[2], p[1]))
    return names, classes


def apply_member_seeds(members, cmap, a, b, moj, cls_moj, off2int, yarn, inter_lines):
    """member-seeds-1.13.2.txt: "mojmap class  mojmap member  yarn class  yarn member" forces one pairing."""
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "member-seeds-1.13.2.txt")
    if not os.path.exists(path):
        return
    ynames, yclasses = yarn
    int2off = {v: k for k, v in off2int.items()}
    # official member of B by (official class, intermediary name)
    off_member = {}
    cur = None
    for line in inter_lines:
        p = line.split("	")
        if p[0] == "c":
            cur = p[1]
        elif p[0] == "" and len(p) >= 5 and p[1] in ("f", "m"):
            off_member[(cur, p[4])] = (p[3], p[2])
    for line in open(path, encoding="utf-8"):
        line = line.split("#")[0].strip()
        if not line:
            continue
        mc, mn, yc, yn = line.split()
        obf_a = cls_moj.get(mc)
        yinter_cls = yclasses.get(yc.replace(".", "/"))
        obf_b = int2off.get(yinter_cls) if yinter_cls else None
        if not obf_a or not obf_b:
            print("member seed: class not found:", line)
            continue
        obf_a = obf_a.replace(".", "/")
        found_a = [(k[1], k[2]) for k, n in moj.items() if k[0] == obf_a and n == mn]
        found_b = [off_member[(obf_b, inter)] for inter, _d, _k in ynames.get((yc.replace(".", "/"), yn), ()) if (obf_b, inter) in off_member]
        if len(found_a) != 1 or len(found_b) != 1:
            print(f"member seed ambiguous or missing ({len(found_a)} x {len(found_b)}):", line)
            continue
        name_a, desc_a = found_a[0]
        name_b, desc_b = found_b[0]
        if desc_a == "F":
            print("member seed: fields aren't supported:", line)
            continue
        for key in [k for k, v in members.items() if (k[0] == obf_a and k[1] == name_a and k[2] == desc_a)
                    or (cmap.get(k[0]) == obf_b and v[0] == name_b and v[1] == desc_b)]:
            del members[key]
        members[(obf_a, name_a, desc_a)] = (name_b, desc_b)
        cmap.setdefault(obf_a, obf_b)


def main():
    work, out = sys.argv[1], sys.argv[2]
    os.makedirs(work, exist_ok=True)
    jar_a = game_download(work, NEW, "client", ".jar")
    jar_b = game_download(work, OLD, "client", ".jar")
    moj_txt = game_download(work, NEW, "client_mappings", ".txt")
    inter = fetch(INTERMEDIARY.format(v=OLD), os.path.join(work, "intermediary.jar"))
    yarn = fetch(LF_YARN.format(v=OLD), os.path.join(work, "yarn.jar"))

    cache = os.path.join(work, "match.json")
    a = classfile.load(jar_a)
    b = classfile.load(jar_b)
    cls_moj, moj = read_mojmap(moj_txt)  # mojmap class -> obf class
    obf_to_moj = {v.replace(".", "/"): k.replace(".", "/") for k, v in cls_moj.items()}

    # Seeds: "mojmap class <TAB> yarn or intermediary class" (dotted or slashed).
    seeds = []
    off2int = {}
    int2off = {}
    for line in read_tiny(inter, "mappings/mappings.tiny"):
        p = line.split("\t")
        if p[0] == "c":
            off2int[p[1]] = p[2]
            int2off[p[2]] = p[1]
    yarn2int = {}
    for line in read_tiny(yarn, "mappings/mappings.tiny"):
        p = line.split("\t")
        if p[0] == "c":
            yarn2int[p[2]] = p[1]
    seed_file = os.path.join(os.path.dirname(os.path.abspath(__file__)), "seeds-1.13.2.txt")
    if os.path.exists(seed_file):
        for line in open(seed_file, encoding="utf-8"):
            line = line.split("#")[0].strip()
            if not line:
                continue
            ma, nb = line.split()
            ma = ma.replace(".", "/")
            nb = nb.replace(".", "/")
            obf_a = cls_moj.get(ma.replace("/", "."))
            obf_b = int2off.get(yarn2int.get(nb, nb))
            if obf_a and obf_b:
                seeds.append((obf_a.replace(".", "/"), obf_b))
            else:
                print("seed unresolved:", line)

    if os.path.exists(cache) and "--rematch" not in sys.argv:
        saved = json.load(open(cache, encoding="utf-8"))
        cmap = saved["classes"]
        members = {(m[0], m[1], m[2]): (m[3], m[4]) for m in saved["members"]}
    else:
        matcher = match.Matcher(a, b)
        cmap, members = matcher.run(seeds)
        json.dump({"classes": cmap, "members": [[k[0], k[1], k[2], v[0], v[1]] for k, v in members.items()]},
                  open(cache, "w", encoding="utf-8"))
    print("classes matched:", len(cmap), "members:", len(members))
    apply_member_seeds(members, cmap, a, b, moj, cls_moj, off2int, yarn_members(yarn), read_tiny(inter, "mappings/mappings.tiny"))

    # B's members in intermediary
    inter_member = {}
    cur = None
    for line in read_tiny(inter, "mappings/mappings.tiny"):
        p = line.split("\t")
        if p[0] == "c":
            cur = p[1]
        elif p[0] == "" and len(p) >= 5 and p[1] in ("f", "m"):
            inter_member[(cur, p[3], p[2])] = p[4]

    def to_inter_desc(desc):
        return re.sub(r"L([^;]+);", lambda m: "L" + off2int.get(m.group(1), m.group(1)) + ";", desc)

    by_class = {}
    for (ca, name, desc), (nb, db) in members.items():
        by_class.setdefault(ca, []).append((name, desc, nb, db))
    skipped = 0
    class_names = {}  # intermediary class -> named
    entries = defaultdict(dict)  # intermediary class -> {(kind, intermediary name, intermediary desc): named}
    for ca, cb in cmap.items():
        ic = off2int.get(cb)
        named_cls = obf_to_moj.get(ca)
        if not ic or not named_cls:
            continue
        if re.search(r"\$\d+$", named_cls):
            # Anonymous classes: numbered in each version's own order; the remapper derives their names.
            continue
        class_names[ic] = named_cls
        for name, desc, nb, db in by_class.get(ca, ()):
            is_field = desc.startswith("F:")
            if name in ("<init>", "<clinit>") or name.startswith("lambda$"):
                continue
            key = (ca, name, "F") if is_field else (ca, name, desc)
            moj_name = moj.get(key)
            inter_name = inter_member.get((cb, nb, db))
            if not moj_name or not inter_name or moj_name == inter_name:
                skipped += 1
                continue
            entries[ic][("f" if is_field else "m", inter_name, to_inter_desc(db))] = moj_name
    # Overriding methods share their intermediary name, and the remapper gives the whole group one name:
    # the most common name among the matched ones wins, and every class declaring the method gets it
    # (a class left out would keep the intermediary name and clash).
    votes = defaultdict(Counter)
    for ic, table in entries.items():
        for (kind, inter_name, idesc), named in table.items():
            if kind == "m":
                votes[inter_name][named] += 1
    declared = defaultdict(list)
    for official, cls in b.items():
        for m in cls.methods:
            inter_name = inter_member.get((official, m.name, m.desc))
            if inter_name and inter_name in votes:
                declared[inter_name].append((off2int.get(official, official), to_inter_desc(m.desc)))
    chosen = {n: v.most_common(1)[0][0] for n, v in votes.items()}
    added = 0
    for inter_name, named in chosen.items():
        for ic, idesc in declared[inter_name]:
            key = ("m", inter_name, idesc)
            if entries[ic].get(key) != named:
                entries[ic][key] = named
                added += 1
    print("method entries added or aligned across overrides:", added)

    # Whatever stays unmatched keeps the name Legacy Yarn gave it (most of what 1.13.2 shares with 1.12),
    # so the sources can reach it where 1.14 differs; the rest stays class_123 / method_456.
    yarn_class = {}
    yarn_member = {}
    cur = None
    for line in read_tiny(yarn, "mappings/mappings.tiny"):
        p = line.split("	")
        if p[0] == "c":
            cur = p[1]
            if p[1] != p[2]:
                yarn_class[p[1]] = p[2]
        elif p[0] == "" and len(p) >= 5 and p[1] in ("f", "m") and p[3] != p[4]:
            yarn_member[(cur, p[1], p[3], p[2])] = p[4]
    taken = set(class_names.values())
    new_classes = 0
    for official, ic in off2int.items():
        if ic in class_names or ic not in yarn_class or "$" in ic:
            continue
        y = yarn_class[ic]
        if y in taken:
            continue
        taken.add(y)
        class_names[ic] = y
        new_classes += 1
    # Inner classes follow their outer class's name when yarn names them; others derive theirs.
    for official, ic in off2int.items():
        if ic in class_names or ic not in yarn_class or "$" not in ic:
            continue
        outer = ic.rsplit("$", 1)[0]
        y = yarn_class[ic]
        if class_names.get(outer, yarn_class.get(outer)) and y not in taken:
            taken.add(y)
            class_names[ic] = y
            new_classes += 1
    new_members = 0
    for (ic, kind, inter_name, idesc), y in yarn_member.items():
        if (kind, inter_name, idesc) in entries[ic]:
            continue
        if kind == "m" and inter_name in votes:
            continue
        entries[ic][(kind, inter_name, idesc)] = y
        new_members += 1
    print(f"yarn names kept for {new_classes} classes and {new_members} members")

    def render():
        out_lines = ["tiny	2	0	intermediary	named"]
        for ic in sorted(set(class_names) | set(entries)):
            out_lines.append(f"c	{ic}	{class_names.get(ic, ic)}")
            for (kind, inter_name, idesc), named in sorted(entries[ic].items()):
                out_lines.append(f"	{kind}	{idesc}	{inter_name}	{named}")
        return out_lines

    import validate
    lines = render()
    for rnd in range(6):
        clashes = validate.find(work, lines)
        if not clashes:
            break
        drop = {inter for _, inter in clashes}
        print(f"clashes in round {rnd}: {len(clashes)}; leaving {len(drop)} methods unmapped")
        for ic in entries:
            for key in [k for k in entries[ic] if k[0] == "m" and k[1] in drop]:
                del entries[ic][key]
        lines = render()
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print("wrote", out, len(lines), "lines; members without a name on one side:", skipped)


if __name__ == "__main__":
    main()
