import struct
import unittest
from prepare_world import prepare_bytes, SETTINGS, LAN_SETTINGS


def tag(kind, name, data):
    name = name.encode()
    return bytes([kind]) + struct.pack("<H", len(name)) + name + data


def level(extra=b""):
    body = tag(10, "", b"".join(tag(kind, name, struct.pack("<i" if kind == 3 else "<b", 2 if kind == 3 else 0))
                               for name, (kind, _) in SETTINGS.items()) + extra + b"\0")
    return struct.pack("<II", 10, len(body)) + body


class Preparation(unittest.TestCase):
    def test_lan_is_opt_in_and_keeps_xbox_broadcast_disabled(self):
        extra=b"".join(tag(kind,name,struct.pack("<i" if kind==3 else "<b",0)) for name,(kind,_) in LAN_SETTINGS.items())
        original=level(extra)
        self.assertTrue(prepare_bytes(original)[0].endswith(extra+b"\0"))
        prepared,evidence=prepare_bytes(original,lan=True)
        self.assertEqual({k:evidence[k]["after"] for k in LAN_SETTINGS},{k:v for k,(_,v) in LAN_SETTINGS.items()})
        self.assertEqual(len(prepared),len(original))
        with self.assertRaises(ValueError):prepare_bytes(level(),lan=True)

    def test_only_known_root_scalars_change(self):
        nested = tag(10, "nested", tag(3, "Difficulty", struct.pack("<i", 3)) + b"\0")
        original = level(nested)
        prepared, evidence = prepare_bytes(original)
        self.assertEqual(len(original), len(prepared))
        self.assertTrue(prepared.endswith(nested + b"\0"))
        self.assertEqual({k: v["after"] for k, v in evidence.items()},
                         {k: v for k, (_, v) in SETTINGS.items()})
        self.assertEqual(prepare_bytes(prepared)[0], prepared)

    def test_malformed_and_duplicate_fields_are_refused(self):
        for invalid in [level()[:-1], level() + b"\0", level(tag(3, "Difficulty", b"\0" * 4)),
                        level(tag(9, "bad", b"\0" + struct.pack("<i", 1)))]:
            with self.assertRaises(ValueError):
                prepare_bytes(invalid)


if __name__ == "__main__":
    unittest.main()
