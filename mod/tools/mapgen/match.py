"""Match the classes and members of two Minecraft versions (obfuscated jars), the way Matcher does.

Anchors are classes with the same unusual string constants; from matched classes the matching spreads
through declaration order, descriptors, supertypes and calls. Returns {class_a: class_b} and
{(class_a, name, desc): (name_b, desc_b)} for fields (desc starts with 'F:') and methods.
"""
import math
import re
import sys
from collections import Counter, defaultdict

DESC_TOKEN = re.compile(r"L([^;]+);")
ACC_STATIC = 0x0008
ACC_ABSTRACT = 0x0400
ACC_INTERFACE = 0x0200
ACC_ENUM = 0x4000
ACC_FINAL = 0x0010
ACC_VISIBILITY = 0x0007


def obf_like(name):
    return len(name) <= 3 and name.islower() or name.startswith(("field_", "method_", "class_"))


class Side:
    def __init__(self, classes):
        self.classes = classes
        self.inner = defaultdict(list)
        for n in classes:
            if "$" in n:
                self.inner[n.rsplit("$", 1)[0]].append(n)
        # who uses a class (as a field type, parameter, return type, superclass)
        self.users = defaultdict(set)
        for c in classes.values():
            for t in self.types_of(c):
                if t in classes and t != c.name:
                    self.users[t].add(c.name)

    @staticmethod
    def descriptor_types(desc):
        return DESC_TOKEN.findall(desc)

    def types_of(self, c):
        out = set()
        if c.super:
            out.add(c.super)
        out.update(c.interfaces)
        for f in c.fields:
            out.update(self.descriptor_types(f.desc))
        for m in c.methods:
            out.update(self.descriptor_types(m.desc))
            for call in m.calls:
                out.add(call[0])
            for ref in m.fields:
                out.add(ref[0])
        return out


class Matcher:
    def __init__(self, a, b, log=print):
        self.a = Side(a)
        self.b = Side(b)
        self.log = log
        self.cmap = {}  # a -> b
        self.cmap_rev = {}
        self.members = {}  # (class_a, name, desc) -> (name_b, desc_b)
        self.members_rev = {}
        self.seeded = set()
        self.strict = False
        self.mvotes = defaultdict(Counter)
        self.anchored = set()

    # ---- descriptor normalisation: matched classes become a shared token, unmatched ones "?" ----

    def norm_a(self, desc):
        return DESC_TOKEN.sub(lambda m: self._tok_a(m.group(1)), desc)

    def norm_b(self, desc):
        return DESC_TOKEN.sub(lambda m: self._tok_b(m.group(1)), desc)

    def _tok_a(self, n):
        if n in self.cmap:
            return "L<" + self.cmap[n] + ">;"
        if n in self.a.classes:
            return "L?;"
        return "L" + n + ";"

    def _tok_b(self, n):
        if n in self.cmap_rev:
            return "L<" + n + ">;"
        if n in self.b.classes:
            return "L?;"
        return "L" + n + ";"

    def unlink(self, ca):
        cb = self.cmap.pop(ca, None)
        if cb is not None:
            self.cmap_rev.pop(cb, None)

    def weak_pairs(self, threshold=0.25):
        """Matched class pairs whose members mostly don't line up: likely wrong."""
        bad = []
        for ca, cb in self.cmap.items():
            A = self.a.classes[ca]
            B = self.b.classes[cb]
            n = max(len(A.methods) + len(A.fields), len(B.methods) + len(B.fields))
            if n < 6:
                continue
            mp = self.align(A.methods, B.methods, lambda x, y: self.method_score(x, y, A, B), 9)
            fp = self.align(A.fields, B.fields, lambda x, y: self.field_score(x, y), 5)
            if (len(mp) + len(fp)) / n < threshold:
                bad.append(ca)
        return bad

    def link(self, ca, cb):
        if ca in self.cmap or cb in self.cmap_rev:
            return False
        self.cmap[ca] = cb
        self.cmap_rev[cb] = ca
        return True

    # ---- anchors ----

    def anchors(self):
        sa = defaultdict(list)
        sb = defaultdict(list)
        for n, c in self.a.classes.items():
            key = frozenset(s for s in c.strings() if len(s) >= 6)
            if len(key) >= 2:
                sa[key].append(n)
        for n, c in self.b.classes.items():
            key = frozenset(s for s in c.strings() if len(s) >= 6)
            if len(key) >= 2:
                sb[key].append(n)
        count = 0
        for key, la in sa.items():
            lb = sb.get(key)
            if lb and len(la) == 1 and len(lb) == 1 and self.link(la[0], lb[0]):
                self.anchored.add(la[0])
                count += 1
        self.log(f"anchors by strings: {count}")
        # Single long unusual strings, unique in both.
        ga = defaultdict(list)
        gb = defaultdict(list)
        for n, c in self.a.classes.items():
            for s in c.strings():
                if len(s) >= 12:
                    ga[s].append(n)
        for n, c in self.b.classes.items():
            for s in c.strings():
                if len(s) >= 12:
                    gb[s].append(n)
        votes = defaultdict(Counter)
        for s, la in ga.items():
            lb = gb.get(s)
            if lb and len(la) == 1 and len(lb) == 1:
                votes[la[0]][lb[0]] += 1
        added = 0
        for ca, counter in votes.items():
            cb, v = counter.most_common(1)[0]
            if v >= 2 and len(counter) == 1 and self.link(ca, cb):
                self.anchored.add(ca)
                added += 1
        self.log(f"anchors by single strings (>= 2 shared): {added}")

    # ---- members ----

    def method_score(self, ma, mb, ca, cb):
        s = 0
        if ma.name == "<init>" or ma.name == "<clinit>":
            if ma.name != mb.name:
                return -100
            s += 3
        elif mb.name in ("<init>", "<clinit>"):
            return -100
        if self.sig_agrees(ma.sig, mb.sig) < 0:
            return -100
        if (ma.access & (ACC_STATIC | ACC_ABSTRACT)) != (mb.access & (ACC_STATIC | ACC_ABSTRACT)):
            return -100
        da = self.norm_a(ma.desc)
        db = self.norm_b(mb.desc)
        # A class neither version can pair says little: shapes that rest on one prove nothing.
        wild = self.strict and ("L?;" in da or "L?;" in db)
        if da == db:
            s += 2 if wild else 8
        else:
            # Same shape if unmatched classes are wildcards.
            wa = re.sub(r"L<[^>]*>;|L\?;", "L?;", da)
            wb = re.sub(r"L<[^>]*>;|L\?;", "L?;", db)
            if wa != wb:
                return -100
            # Differ only in matched/unmatched mix.
            ta = DESC_TOKEN.findall(da)
            tb = DESC_TOKEN.findall(db)
            clash = sum(1 for x, y in zip(ta, tb) if x != y and x.startswith("<") and y.startswith("<"))
            if clash:
                return -100
            s += 1 if wild else 4
        self._shape = s
        s += 1 if (ma.access & ACC_VISIBILITY) == (mb.access & ACC_VISIBILITY) else -1
        if not obf_like(ma.name) and ma.name == mb.name:
            s += 6
        elif not obf_like(ma.name) and not obf_like(mb.name) and ma.name != mb.name and len(ma.name) > 4:
            s -= 4
        both = ma.strings & mb.strings
        s += min(15, 5 * len(both))
        s -= min(6, 2 * len(ma.strings ^ mb.strings))
        if ma.size and mb.size:
            r = min(ma.size, mb.size) / max(ma.size, mb.size)
            s += int(4 * r) - 2
        elif ma.size or mb.size:
            s -= 3
        # Same java.* calls
        ja = Counter(f"{c[0]}.{c[1]}{c[2]}" for c in ma.calls if not c[0] in self.a.classes)
        jb = Counter(f"{c[0]}.{c[1]}{c[2]}" for c in mb.calls if not c[0] in self.b.classes)
        s += min(16, 4 * sum((ja & jb).values()))
        s -= min(8, 2 * sum(((ja - jb) + (jb - ja)).values()))
        ia_ints = set(ma.ints)
        ib_ints = set(mb.ints)
        s += min(4, len(ia_ints & ib_ints))
        # Calls that stay in a matched pair of classes
        ia = Counter((self.cmap.get(c[0]), c[1] if not obf_like(c[1]) else "", self.norm_a(c[2])) for c in ma.calls if c[0] in self.cmap)
        ib = Counter((c[0] and self.cmap_rev.get(c[0]) and c[0], c[1] if not obf_like(c[1]) else "", self.norm_b(c[2])) for c in mb.calls if c[0] in self.cmap_rev)
        s += min(6, sum((ia & ib).values()))
        s += min(21, 7 * self.mvotes[(ca.name, ma.name, ma.desc)][(cb.name, mb.name, mb.desc)])
        return s

    SIG_CLASS = re.compile(r"L([^;<]+)(?=[;<])")

    def sig_agrees(self, sa, sb):
        """Generic signatures, when both members have them, must have the same shape. 1 agree, 0 unknown, -1 differ."""
        if not sa or not sb:
            return 0
        ta = self.SIG_CLASS.sub(lambda m: "L<?>" if m.group(1) in self.a.classes else "L" + m.group(1), sa)
        tb = self.SIG_CLASS.sub(lambda m: "L<?>" if m.group(1) in self.b.classes else "L" + m.group(1), sb)
        if ta != tb:
            return -1
        # Same shape: do the type arguments that both sides know say the same?
        xa = self.SIG_CLASS.sub(lambda m: "L<" + self.cmap.get(m.group(1), "?") + ">" if m.group(1) in self.a.classes else "L" + m.group(1), sa)
        xb = self.SIG_CLASS.sub(lambda m: "L<" + (m.group(1) if m.group(1) in self.cmap_rev else "?") + ">" if m.group(1) in self.b.classes else "L" + m.group(1), sb)
        if xa == xb:
            return 1
        ka = re.findall(r"L<([^>]*)>", xa)
        kb = re.findall(r"L<([^>]*)>", xb)
        if any(x != "?" and y != "?" and x != y for x, y in zip(ka, kb)):
            return -1
        return 0

    def field_score(self, fa, fb, ca=None, cb=None):
        if (fa.access & ACC_STATIC) != (fb.access & ACC_STATIC):
            return -100
        if self.sig_agrees(fa.sig, fb.sig) < 0:
            return -100
        da = self.norm_a(fa.desc)
        db = self.norm_b(fb.desc)
        wild = self.strict and ("L?;" in da or "L?;" in db)
        if da == db:
            s = 2 if wild else 8
        else:
            wa = re.sub(r"L<[^>]*>;|L\?;", "L?;", da)
            wb = re.sub(r"L<[^>]*>;|L\?;", "L?;", db)
            if wa != wb:
                return -100
            ta = DESC_TOKEN.findall(da)
            tb = DESC_TOKEN.findall(db)
            if any(x != y and x.startswith("<") and y.startswith("<") for x, y in zip(ta, tb)):
                return -100
            s = 1 if wild else 4
        # Visibility rarely changes between versions.
        s += 2 if (fa.access & ACC_VISIBILITY) == (fb.access & ACC_VISIBILITY) else -2
        if ca is not None:
            s += min(21, 7 * self.mvotes[(ca, fa.name, "F:" + fa.desc)][(cb, fb.name, "F:" + fb.desc)])
        if (fa.access & ACC_FINAL) == (fb.access & ACC_FINAL):
            s += 1
        if not obf_like(fa.name) and fa.name == fb.name:
            s += 6
        return s

    @staticmethod
    def align(items_a, items_b, score, floor):
        """Pair members: best scores first (a pair must be the best choice for both sides, order breaks ties).
        Returns [(i, j)] sorted by i."""
        n, m = len(items_a), len(items_b)
        if not n or not m:
            return []
        cand = []
        for i in range(n):
            for j in range(m):
                v = score(items_a[i], items_b[j])
                if v >= floor:
                    # Declaration order is a weak hint: members stay roughly where they were.
                    v2 = v + 3.0 * (1.0 - abs(i / n - j / m))
                    cand.append((v2, i, j))
        cand.sort(reverse=True)
        best_a = {}
        best_b = {}
        for v, i, j in cand:
            best_a.setdefault(i, v)
            best_b.setdefault(j, v)
        used_a = set()
        used_b = set()
        pairs = []
        for v, i, j in cand:
            if i in used_a or j in used_b:
                continue
            used_a.add(i)
            used_b.add(j)
            pairs.append((i, j))
        pairs.sort()
        return pairs

    def match_members(self, ca, cb):
        """Align one matched class pair's members; returns type votes for classes not matched yet."""
        A = self.a.classes[ca]
        B = self.b.classes[cb]
        votes = []
        mp = self.align(A.methods, B.methods, lambda x, y: self.method_score(x, y, A, B), 9)
        fp = self.align(A.fields, B.fields, lambda x, y: self.field_score(x, y, ca, cb), 5)
        for i, j in fp:
            fa, fb = A.fields[i], B.fields[j]
            self.members[(ca, fa.name, "F:" + fa.desc)] = (fb.name, fb.desc)
            votes += self.type_votes(fa.desc, fb.desc)
        for i, j in mp:
            ma, mb = A.methods[i], B.methods[j]
            self.members[(ca, ma.name, ma.desc)] = (mb.name, mb.desc)
            votes += self.type_votes(ma.desc, mb.desc)
            self.usage_votes(ma, mb)
            # Calls and field uses into other classes at the same position
            ra = [c for c in ma.calls if c[0] in self.a.classes and c[0] != ca]
            rb = [c for c in mb.calls if c[0] in self.b.classes and c[0] != cb]
            if len(ra) == len(rb) and len(ra) <= 12:
                for x, y in zip(ra, rb):
                    if self.norm_a(x[2]).replace("L?;", "L?;") == self.norm_b(y[2]) or \
                            re.sub(r"L<[^>]*>;|L\?;", "L?;", self.norm_a(x[2])) == re.sub(r"L<[^>]*>;|L\?;", "L?;", self.norm_b(y[2])):
                        votes.append((x[0], y[0], 1))
        if A.super and B.super and A.super in self.a.classes and B.super in self.b.classes:
            votes.append((A.super, B.super, 3))
        if len(A.interfaces) == len(B.interfaces):
            for x, y in zip(A.interfaces, B.interfaces):
                if x in self.a.classes and y in self.b.classes:
                    votes.append((x, y, 3))
        return votes

    def usage_votes(self, ma, mb):
        """Matched methods use the same members in the same order: those members belong together."""
        for kind in ("calls", "fields"):
            ra = []
            for ref in getattr(ma, kind):
                if ref[0] in self.cmap and ref not in ra:
                    ra.append(ref)
            rb = []
            for ref in getattr(mb, kind):
                if ref[0] in self.cmap_rev and ref not in rb:
                    rb.append(ref)
            if len(ra) != len(rb) or len(ra) > 24:
                continue
            for x, y in zip(ra, rb):
                if self.cmap.get(x[0]) != y[0]:
                    continue
                if kind == "calls":
                    if re.sub(r"L<[^>]*>;|L\?;", "L?;", self.norm_a(x[2])) != re.sub(r"L<[^>]*>;|L\?;", "L?;", self.norm_b(y[2])):
                        continue
                    self.mvotes[(x[0], x[1], x[2])][(y[0], y[1], y[2])] += 1
                else:
                    self.mvotes[(x[0], x[1], "F:" + x[2])][(y[0], y[1], "F:" + y[2])] += 1

    def type_votes(self, da, db):
        ta = DESC_TOKEN.findall(da)
        tb = DESC_TOKEN.findall(db)
        if len(ta) != len(tb):
            return []
        shape_a = DESC_TOKEN.sub("L?;", da)
        shape_b = DESC_TOKEN.sub("L?;", db)
        if shape_a != shape_b:
            return []
        out = []
        for x, y in zip(ta, tb):
            if x in self.a.classes and y in self.b.classes:
                out.append((x, y, 2))
        return out

    # ---- classes without a neighbour that voted for them: by what they contain ----

    def class_similarity(self, ca, cb):
        A = self.a.classes[ca]
        B = self.b.classes[cb]
        if (A.access & ACC_INTERFACE) != (B.access & ACC_INTERFACE) or (A.access & ACC_ENUM) != (B.access & ACC_ENUM):
            return 0
        mp = self.align(A.methods, B.methods, lambda x, y: self.method_score(x, y, A, B), 9)
        # What the members share beyond having the same shape: strings, calls, numbers, names.
        total = 0
        for i, j in mp:
            score = self.method_score(A.methods[i], B.methods[j], A, B)
            total += max(0, score - self._shape)
        # Penalise very different sizes.
        na = len(A.methods) + len(A.fields)
        nb = len(B.methods) + len(B.fields)
        if na and nb:
            total = total * min(na, nb) / max(na, nb)
        sa = self.a.classes[ca].super
        sb = self.b.classes[cb].super
        if sa in self.cmap and self.cmap[sa] == sb:
            total += 6
        return total

    def candidates(self):
        """Unmatched classes of a, each with the unmatched classes of b that share a neighbour or rare string."""
        strings_b = defaultdict(set)
        for n, c in self.b.classes.items():
            if n in self.cmap_rev:
                continue
            for s in c.strings():
                if len(s) >= 5:
                    strings_b[s].add(n)
        strings_a = defaultdict(set)
        for n, c in self.a.classes.items():
            if n not in self.cmap:
                for s in c.strings():
                    if len(s) >= 5:
                        strings_a[s].add(n)
        out = {}
        for n, c in self.a.classes.items():
            if n in self.cmap:
                continue
            cand = Counter()
            for s in c.strings():
                bs = strings_b.get(s)
                if bs and len(bs) <= 4 and len(strings_a[s]) <= 4:
                    for x in bs:
                        cand[x] += 2
            # Neighbours: classes that use / are used by matched classes
            for u in self.a.users.get(n, ()):
                if u in self.cmap:
                    for x in self.b.users:
                        pass
            for t in self.a.types_of(c):
                if t in self.cmap:
                    tb = self.cmap[t]
                    for x in self.b.users.get(tb, ()):
                        if x not in self.cmap_rev:
                            cand[x] += 1
            # Rare shared neighbours: the unmatched classes the counterparts of n's users all mention.
            counterparts = [self.cmap[u] for u in self.a.users.get(n, ()) if u in self.cmap]
            if counterparts:
                near = Counter()
                for ub in counterparts:
                    for t in self.b.types_of(self.b.classes[ub]):
                        if t in self.b.classes and t not in self.cmap_rev:
                            near[t] += 1
                total = len(self.b.classes)
                for t, k in near.items():
                    cand[t] += 3.0 * k * math.log(total / (1 + len(self.b.users.get(t, ())))) / len(counterparts)
            if cand:
                out[n] = [x for x, _ in cand.most_common(14)]
        return out

    def similarity_phase(self):
        cands = self.candidates()
        scores = {}
        for ca, lst in cands.items():
            sc = sorted(((self.class_similarity(ca, cb), cb) for cb in lst), reverse=True)
            if sc and sc[0][0] >= 24:
                second = sc[1][0] if len(sc) > 1 else 0
                if sc[0][0] >= 1.6 * second:
                    scores[ca] = sc[0]
        # Mutual best
        by_b = defaultdict(list)
        for ca, (s, cb) in scores.items():
            by_b[cb].append((s, ca))
        added = 0
        for cb, lst in by_b.items():
            lst.sort(reverse=True)
            if len(lst) > 1 and lst[0][0] < 1.6 * lst[1][0]:
                continue
            if self.link(lst[0][1], cb):
                added += 1
        return added

    # ---- driver ----

    def run(self, seeds=()):
        for ca, cb in seeds:
            if ca in self.a.classes and cb in self.b.classes:
                self.link(ca, cb)
                self.seeded.add(ca)
            else:
                self.log(f"seed not found: {ca} {cb}")
        self.anchors()
        done = set()
        for rnd in range(40):
            self.members = {}
            allvotes = Counter()
            for ca, cb in list(self.cmap.items()):
                for x, y, w in self.match_members(ca, cb):
                    if x not in self.cmap and y not in self.cmap_rev:
                        allvotes[(x, y)] += w
            # Accept the best pairs first; a class takes one partner only.
            by_a = defaultdict(list)
            for (x, y), v in allvotes.items():
                by_a[x].append((v, y))
            by_b = defaultdict(list)
            for (x, y), v in allvotes.items():
                by_b[y].append((v, x))
            added = 0
            for (x, y), v in sorted(allvotes.items(), key=lambda kv: -kv[1]):
                if x in self.cmap or y in self.cmap_rev:
                    continue
                best_a = max(by_a[x])[0]
                best_b = max(by_b[y])[0]
                if v < best_a or v < best_b:
                    continue
                second_a = sorted((t[0] for t in by_a[x]), reverse=True)[1:2]
                if second_a and second_a[0] == v:
                    continue
                if v >= 2 and self.link(x, y):
                    added += 1
            self.log(f"round {rnd}: +{added} classes, total {len(self.cmap)}")
            if not added:
                break
        for phase in range(6):
            added = self.similarity_phase()
            self.log(f"similarity phase {phase}: +{added} classes, total {len(self.cmap)}")
            if not added:
                break
            for rnd in range(10):
                allvotes = Counter()
                for ca, cb in list(self.cmap.items()):
                    for x, y, w in self.match_members(ca, cb):
                        if x not in self.cmap and y not in self.cmap_rev:
                            allvotes[(x, y)] += w
                n = 0
                for (x, y), v in sorted(allvotes.items(), key=lambda kv: -kv[1]):
                    if x in self.cmap or y in self.cmap_rev:
                        continue
                    if v >= 3 and self.link(x, y):
                        n += 1
                self.log(f"  propagate {rnd}: +{n}")
                if not n:
                    break
        for again in range(2):
            bad = [ca for ca in self.weak_pairs() if ca not in self.seeded and ca not in self.anchored]
            self.log(f"weak pairs dropped: {len(bad)}")
            if not bad:
                break
            for ca in bad:
                self.unlink(ca)
            for phase in range(4):
                added = self.similarity_phase()
                self.log(f"  rematch {phase}: +{added}")
                if not added:
                    break
        # The final member pairs: where a shape rests on a class neither version has, it needs more than the shape.
        self.strict = True
        for round_ in range(3):
            # Each pass learns which members the matched methods share, and the next one pairs by that.
            self.members = {}
            for ca, cb in self.cmap.items():
                self.match_members(ca, cb)
        return self.cmap, self.members
