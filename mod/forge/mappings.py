"""Compose a Yarn -> SRG mapping for Minecraft 1.8.9, to remap the Legacy Fabric build onto Forge.

Forge 1.8.9 runs mods against SRG names (net/minecraft/client/Minecraft, func_71410_x, field_71439_g);
the Arctic Client is written against Yarn. Both start from the obfuscated names, so:

    yarn tiny (official -> intermediary -> named)   joined from
    MCP joined.srg (official -> srg)

give named -> srg, written as a Tiny v1 file for tiny-remapper.
"""
import re
import sys
import zipfile
from pathlib import Path


def read_yarn(path):
    """-> (classes, fields, methods): official class -> (named class); (official class, name, desc) -> named."""
    classes, fields, methods = {}, {}, {}
    current = None
    for line in Path(path).read_text(encoding="utf-8").splitlines()[1:]:
        parts = line.split("\t")
        if parts[0] == "c":
            # c official intermediary named
            current = parts[1]
            classes[current] = parts[3] if len(parts) > 3 and parts[3] else parts[2]
        elif parts[0] == "" and current and len(parts) >= 5 and parts[1] in ("f", "m"):
            # <tab>f/m desc official intermediary named
            desc, official, intermediary = parts[2], parts[3], parts[4]
            named = parts[5] if len(parts) > 5 and parts[5] else intermediary
            (fields if parts[1] == "f" else methods)[(current, official, desc)] = named
    return classes, fields, methods


def read_srg(path):
    """-> class, field and method maps from the obfuscated names."""
    classes, fields, methods = {}, {}, {}
    text = path.read_text(encoding="utf-8") if hasattr(path, "read_text") else path
    for line in text.splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "CL:":
            classes[parts[1]] = parts[2]
        elif parts[0] == "FD:":
            owner, _, name = parts[1].rpartition("/")
            fields[(owner, name)] = parts[2].rpartition("/")[2]
        elif parts[0] == "MD:":
            owner, _, name = parts[1].rpartition("/")
            methods[(owner, name, parts[2])] = parts[3].rpartition("/")[2]
    return classes, fields, methods


def remap_desc(desc, class_map):
    def swap(match):
        return "L" + class_map.get(match.group(1), match.group(1)) + ";"
    return re.sub(r"L([^;]+);", swap, desc)


def compose(yarn_path, srg_zip, out_path):
    yarn_classes, yarn_fields, yarn_methods = read_yarn(yarn_path)
    with zipfile.ZipFile(srg_zip) as z:
        srg_classes, srg_fields, srg_methods = read_srg(z.read("joined.srg").decode("utf-8"))
    # official class -> named class; named class -> srg class (an unmapped class keeps its obfuscated name's srg)
    named_to_srg = {}
    official_to_named = {}
    for official, named in yarn_classes.items():
        official_to_named[official] = named
        named_to_srg[named] = srg_classes.get(official, official)
    out = ["v1\tnamed\tsrg"]
    for named, srg in sorted(named_to_srg.items()):
        out.append(f"CLASS\t{named}\t{srg}")
    for (official, name, desc), named_name in sorted(yarn_fields.items()):
        srg = srg_fields.get((official, name))
        if not srg or srg == named_name:
            continue
        out.append(f"FIELD\t{yarn_classes[official]}\t{remap_desc(desc, official_to_named)}\t{named_name}\t{srg}")
    for (official, name, desc), named_name in sorted(yarn_methods.items()):
        srg = srg_methods.get((official, name, desc))
        if not srg or srg == named_name:
            continue
        out.append(f"METHOD\t{yarn_classes[official]}\t{remap_desc(desc, official_to_named)}\t{named_name}\t{srg}")
    Path(out_path).write_text("\n".join(out) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(named_to_srg)} classes, {sum(1 for l in out if l.startswith('FIELD'))} fields, "
          f"{sum(1 for l in out if l.startswith('METHOD'))} methods -> {out_path}")


if __name__ == "__main__":
    compose(*map(Path, sys.argv[1:4]))
