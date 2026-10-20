"""Find mappings the remapper would reject: two members of a class (or of a class and its supers) that end up
with the same name and descriptor. Usage: python validate.py <work dir> <tiny file>"""
import os
import re
import sys
import zipfile
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import classfile  # noqa: E402


_B = {}


def find(work, tiny_lines):
    """Clashes in a mapping (Tiny lines): [(message, intermediary name of the method to leave unmapped)]."""
    if work not in _B:
        _B[work] = classfile.load(os.path.join(work, "1.13.2-client.jar"))
    b = _B[work]
    out = []
    inter = zipfile.ZipFile(os.path.join(work, "intermediary.jar")).read("mappings/mappings.tiny").decode().splitlines()
    off2int = {}
    imem = {}
    cur = None
    for line in inter:
        p = line.split("\t")
        if p[0] == "c":
            cur = p[1]
            off2int[p[1]] = p[2]
        elif p[0] == "" and p[1] in ("f", "m"):
            imem[(cur, p[3], p[2])] = p[4]
    cname = {}
    mname = {}
    cur = None
    for line in tiny_lines:
        p = line.rstrip("\n").split("\t")
        if p[0] == "c":
            cur = p[1]
            cname[p[1]] = p[2]
        elif p[0] == "" and p[1] in ("f", "m"):
            mname[(cur, p[3], p[2])] = p[4]
    inv = {v: k for k, v in off2int.items()}

    def named_desc(desc):
        return re.sub(r"L([^;]+);", lambda m: "L" + cname.get(off2int.get(m.group(1), m.group(1)), off2int.get(m.group(1), m.group(1))) + ";", desc)

    bad = 0
    finals = {}
    for name, c in b.items():
        ic = off2int.get(name, name)
        methods = defaultdict(list)
        for m in c.methods:
            if m.name in ("<init>", "<clinit>"):
                continue
            inter_name = imem.get((name, m.name, m.desc), m.name)
            # desc in intermediary space, for the lookup in the tiny
            idesc = re.sub(r"L([^;]+);", lambda x: "L" + off2int.get(x.group(1), x.group(1)) + ";", m.desc)
            final = mname.get((ic, idesc, inter_name), inter_name) if False else mname.get((ic, inter_name, idesc), inter_name)
            methods[(final, named_desc(m.desc))].append(m)
        fields = defaultdict(list)
        for f in c.fields:
            inter_name = imem.get((name, f.name, f.desc), f.name)
            idesc = re.sub(r"L([^;]+);", lambda x: "L" + off2int.get(x.group(1), x.group(1)) + ";", f.desc)
            final = mname.get((ic, inter_name, idesc), inter_name)
            fields[(final, named_desc(f.desc))].append(f)
        for key, lst in methods.items():
            if len(lst) > 1:
                bad += 1
                for x in lst:
                    out.append((f"method clash in {cname.get(ic, ic)} {key[0]}", imem.get((name, x.name, x.desc), x.name)))
        for key, lst in fields.items():
            if len(lst) > 1:
                bad += 1
        finals[name] = {k: imem.get((name, v[0].name, v[0].desc), v[0].name) for k, v in methods.items()}
    # A subclass method that gets the same final name and descriptor as a different method of a super class.
    supers = {}
    for name, c in b.items():
        chain = []
        sup = c.super
        while sup in b and len(chain) < 12:
            chain.append(sup)
            sup = b[sup].super
        for itf in c.interfaces:
            if itf in b:
                chain.append(itf)
        supers[name] = chain
    for name in b:
        for sup in supers[name]:
            for key, inter in finals[name].items():
                other = finals[sup].get(key)
                if other is not None and other != inter:
                    bad += 1
                    out.append((f"hierarchy clash {cname.get(off2int.get(name, name), name)} vs {cname.get(off2int.get(sup, sup), sup)} {key[0]}", inter))
                    out.append(("  and", other))
    return out


def main():
    work, tiny = sys.argv[1], sys.argv[2]
    found = find(work, open(tiny, encoding="utf-8").read().splitlines())
    for message, inter in found:
        print(message, inter)
    print("clashes:", len(found))


if __name__ == "__main__":
    main()
