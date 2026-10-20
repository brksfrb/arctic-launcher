"""How many of the members the adapter's sources mention (by name) did the matcher pair? Usage: python memcov.py <work dir>"""
import glob, json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import generate
work = sys.argv[1]
saved = json.load(open(os.path.join(work, "match.json"), encoding="utf-8"))
members = {(m[0], m[1], m[2]) for m in saved["members"]}
cmap = saved["classes"]
cls_moj, moj = generate.read_mojmap(os.path.join(work, "1.14.4-client_mappings.txt"))
obf_to_moj = {v.replace(".", "/"): k for k, v in cls_moj.items()}
src = ""
for f in glob.glob(r"C:\Users\burak\RustroverProjects\arctic-launcher\mod\versions\fabric\src\main\java\**\*.java", recursive=True):
    src += open(f, encoding="utf-8").read()
tokens = set(re.findall(r"\.(\w+)\(", src)) | set(re.findall(r"\b(\w+)\(", src))
fields = set(re.findall(r"\.(\w+)\b", src))
matched_names = set()
for (oc, name, desc) in members:
    key = (oc, name, "F") if desc.startswith("F:") else (oc, name, desc)
    n = moj.get(key)
    if n: matched_names.add(n)
want = {}
for (oc, obf, desc), n in moj.items():
    if oc in cmap or oc in obf_to_moj:
        c = obf_to_moj.get(oc)
        if c and c.startswith(("net.minecraft.client", "com.mojang.blaze3d")) and (n in tokens if desc != "F" else n in fields):
            want.setdefault((oc, n), 0)
tot = len(want)
hit = sum(1 for (oc, n) in want if oc in cmap and any(True for (c, nn, d) in members if c == oc and moj.get((c, nn, d if not d.startswith("F:") else "F")) == n))
print("members mentioned in the sources, in client classes:", tot, "paired:", hit)
miss = sorted((obf_to_moj[oc], n) for (oc, n) in want if not (oc in cmap and any(True for (c, nn, d) in members if c == oc and moj.get((c, nn, d if not d.startswith("F:") else "F")) == n)))
import collections
by = collections.defaultdict(list)
for c, n in miss: by[c].append(n)
for c in sorted(by, key=lambda k: -len(by[k]))[:40]:
    print(c.split('.')[-1], len(by[c]), by[c][:14], "(class matched)" if cls_moj[c].replace('.','/') in cmap else "(CLASS UNMATCHED)")
