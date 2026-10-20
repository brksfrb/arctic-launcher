"""A small Java class file reader: just what matching two Minecraft versions needs.

Reads a jar's classes into plain objects: names, supertypes, fields, methods and, per method, the
string constants it loads, the methods and fields it uses, and its size. No dependencies.
"""
import struct
import zipfile

# Bytes each opcode takes after the opcode itself (-1: special).
_OPLEN = {}
for _op in range(256):
    _OPLEN[_op] = 0
for _op in (0x10, 0x12, 0x15, 0x16, 0x17, 0x18, 0x19, 0x36, 0x37, 0x38, 0x39, 0x3A, 0xA9, 0xBC):
    _OPLEN[_op] = 1  # bipush ldc xload xstore ret newarray
for _op in (0x11, 0x13, 0x14, 0x84, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xBB, 0xBD, 0xC0, 0xC1, 0xC6, 0xC7):
    _OPLEN[_op] = 2  # sipush ldc_w ldc2_w iinc get/put/invoke new anewarray checkcast instanceof ifnull ifnonnull
for _op in range(0x99, 0xA9):
    _OPLEN[_op] = 2  # if*, goto, jsr
_OPLEN[0xC5] = 3  # multianewarray
_OPLEN[0xB9] = 4  # invokeinterface
_OPLEN[0xBA] = 4  # invokedynamic
_OPLEN[0xC8] = 4  # goto_w
_OPLEN[0xC9] = 4  # jsr_w
_OPLEN[0xAA] = -1  # tableswitch
_OPLEN[0xAB] = -1  # lookupswitch
_OPLEN[0xC4] = -1  # wide


class Method:
    __slots__ = ("name", "desc", "access", "strings", "calls", "fields", "size", "ints")

    def __init__(self, name, desc, access):
        self.name = name
        self.desc = desc
        self.access = access
        self.strings = set()
        self.calls = []
        self.fields = []
        self.size = 0
        self.ints = []


class Field:
    __slots__ = ("name", "desc", "access")

    def __init__(self, name, desc, access):
        self.name = name
        self.desc = desc
        self.access = access


class Class:
    __slots__ = ("name", "super", "interfaces", "access", "fields", "methods", "inner_of")

    def __init__(self):
        self.fields = []
        self.methods = []
        self.inner_of = None

    def strings(self):
        out = set()
        for m in self.methods:
            out |= m.strings
        return out


def _utf8(b):
    # Modified UTF-8; Minecraft's strings are plain enough.
    return b.decode("utf-8", "replace")


def parse(data):
    pos = 8
    (count,) = struct.unpack_from(">H", data, pos)
    pos += 2
    pool = [None] * count
    i = 1
    while i < count:
        tag = data[pos]
        pos += 1
        if tag == 1:
            (n,) = struct.unpack_from(">H", data, pos)
            pool[i] = ("u", _utf8(data[pos + 2:pos + 2 + n]))
            pos += 2 + n
        elif tag == 3:
            pool[i] = ("i", struct.unpack_from(">i", data, pos)[0])
            pos += 4
        elif tag == 4:
            pool[i] = ("f", struct.unpack_from(">f", data, pos)[0])
            pos += 4
        elif tag == 5:
            pool[i] = ("l", struct.unpack_from(">q", data, pos)[0])
            pos += 8
            i += 1
        elif tag == 6:
            pool[i] = ("d", struct.unpack_from(">d", data, pos)[0])
            pos += 8
            i += 1
        elif tag in (7, 8, 16, 19, 20):
            pool[i] = ({7: "c", 8: "s", 16: "mt", 19: "mod", 20: "pkg"}[tag], struct.unpack_from(">H", data, pos)[0])
            pos += 2
        elif tag in (9, 10, 11, 12, 17, 18):
            a, b = struct.unpack_from(">HH", data, pos)
            pool[i] = ({9: "fr", 10: "mr", 11: "ir", 12: "nt", 17: "dyn", 18: "indy"}[tag], a, b)
            pos += 4
        elif tag == 15:
            pool[i] = ("mh", data[pos], struct.unpack_from(">H", data, pos + 1)[0])
            pos += 3
        else:
            raise ValueError(f"constant pool tag {tag}")
        i += 1

    def u(idx):
        return pool[idx][1]

    def cname(idx):
        return u(pool[idx][1])

    def ref(idx):
        kind, c, nt = pool[idx]
        return (cname(c), u(pool[nt][1]), u(pool[nt][2]))

    cls = Class()
    cls.access, this, sup, icount = struct.unpack_from(">HHHH", data, pos)
    pos += 8
    cls.name = cname(this)
    cls.super = cname(sup) if sup else None
    cls.interfaces = []
    for _ in range(icount):
        (ix,) = struct.unpack_from(">H", data, pos)
        pos += 2
        cls.interfaces.append(cname(ix))

    def attrs(p):
        out = []
        (n,) = struct.unpack_from(">H", data, p)
        p += 2
        for _ in range(n):
            nameix, length = struct.unpack_from(">HI", data, p)
            out.append((u(nameix), p + 6, length))
            p += 6 + length
        return out, p

    (fcount,) = struct.unpack_from(">H", data, pos)
    pos += 2
    for _ in range(fcount):
        access, nameix, descix = struct.unpack_from(">HHH", data, pos)
        pos += 6
        _, pos = attrs(pos)
        cls.fields.append(Field(u(nameix), u(descix), access))
    (mcount,) = struct.unpack_from(">H", data, pos)
    pos += 2
    for _ in range(mcount):
        access, nameix, descix = struct.unpack_from(">HHH", data, pos)
        pos += 6
        method = Method(u(nameix), u(descix), access)
        alist, pos = attrs(pos)
        for aname, start, length in alist:
            if aname == "Code":
                maxs, maxl, codelen = struct.unpack_from(">HHI", data, start)
                code = data[start + 8:start + 8 + codelen]
                method.size = codelen
                _scan(code, pool, u, ref, cname, method)
        cls.methods.append(method)
    return cls


def _scan(code, pool, u, ref, cname, method):
    i = 0
    n = len(code)
    while i < n:
        op = code[i]
        if op in (0x12, 0x13, 0x14):
            idx = code[i + 1] if op == 0x12 else (code[i + 1] << 8) | code[i + 2]
            ent = pool[idx]
            if ent[0] == "s":
                method.strings.add(u(ent[1]))
            elif ent[0] in ("i", "f", "l", "d"):
                method.ints.append(ent[1])
        elif op in (0xB2, 0xB3, 0xB4, 0xB5):
            method.fields.append(ref((code[i + 1] << 8) | code[i + 2]))
        elif op in (0xB6, 0xB7, 0xB8, 0xB9):
            method.calls.append(ref((code[i + 1] << 8) | code[i + 2]))
        elif op == 0x10:
            method.ints.append(code[i + 1] - 256 if code[i + 1] > 127 else code[i + 1])
        elif op == 0x11:
            v = (code[i + 1] << 8) | code[i + 2]
            method.ints.append(v - 65536 if v > 32767 else v)
        ln = _OPLEN[op]
        if ln >= 0:
            i += 1 + ln
        elif op == 0xC4:
            i += 6 if code[i + 1] == 0x84 else 4
        elif op == 0xAA:
            j = (i + 4) & ~3
            lo = struct.unpack_from(">i", code, j + 4)[0]
            hi = struct.unpack_from(">i", code, j + 8)[0]
            i = j + 12 + 4 * (hi - lo + 1)
        else:
            j = (i + 4) & ~3
            npairs = struct.unpack_from(">i", code, j + 4)[0]
            i = j + 8 + 8 * npairs


def load(path):
    classes = {}
    with zipfile.ZipFile(path) as z:
        for name in z.namelist():
            if name.endswith(".class") and not name.startswith("META-INF"):
                c = parse(z.read(name))
                classes[c.name] = c
    return classes
