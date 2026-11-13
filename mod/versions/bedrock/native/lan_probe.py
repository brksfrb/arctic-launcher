"""Kernel UDP ownership checks for the isolated installed-game recording probe."""
import ctypes
import socket


class UdpRow(ctypes.Structure):
    _fields_ = [("address", ctypes.c_uint32), ("port", ctypes.c_uint32), ("pid", ctypes.c_uint32)]


class UdpTable(ctypes.Structure):
    _fields_ = [("count", ctypes.c_uint32), ("rows", UdpRow * 1)]


class Udp6Row(ctypes.Structure):
    _fields_ = [("address", ctypes.c_ubyte * 16), ("scope", ctypes.c_uint32),
                ("port", ctypes.c_uint32), ("pid", ctypes.c_uint32)]


class Udp6Table(ctypes.Structure):
    _fields_ = [("count", ctypes.c_uint32), ("rows", Udp6Row * 1)]


def owned_udp_endpoints(pid):
    # SDK MIB_UDPTABLE_OWNER_PID / MIB_UDPROW_OWNER_PID. Use ctypes' actual
    # alignment rather than assuming row padding from a packed wire structure.
    if not isinstance(pid, int) or not 1 <= pid <= 0xffffffff:
        raise ValueError("a live owned process ID is required")
    query = ctypes.WinDLL("iphlpapi").GetExtendedUdpTable
    query.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32), ctypes.c_int,
                      ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
    query.restype = ctypes.c_uint32
    return (_owned_table(query, pid, socket.AF_INET, UdpRow, UdpTable)
            + _owned_table(query, pid, socket.AF_INET6, Udp6Row, Udp6Table))


def _owned_table(query, pid, family, row_type, table_type):
    size = ctypes.c_uint32()
    result = query(None, ctypes.byref(size), False, family, 1, 0)
    for _ in range(3):
        if result not in (0, 122):
            raise OSError(result, "GetExtendedUdpTable failed")
        if not table_type.rows.offset <= size.value <= 16 * 1024 * 1024:
            raise ValueError("invalid UDP owner table size")
        buffer = ctypes.create_string_buffer(size.value)
        result = query(buffer, ctypes.byref(size), False, family, 1, 0)
        if result == 122:
            continue
        if result:
            raise OSError(result, "GetExtendedUdpTable failed")
        count = ctypes.c_uint32.from_buffer_copy(buffer).value
        if count > (len(buffer) - table_type.rows.offset) // ctypes.sizeof(row_type):
            raise ValueError("truncated UDP owner table")
        rows = []
        for i in range(count):
            row = row_type.from_buffer_copy(buffer, table_type.rows.offset + i * ctypes.sizeof(row_type))
            if row.pid == pid:
                address = (socket.inet_ntoa(bytes(ctypes.c_uint32(row.address))) if family == socket.AF_INET
                           else socket.inet_ntop(socket.AF_INET6, bytes(row.address)))
                rows.append({"pid": pid, "address": address, "port": socket.ntohs(row.port & 0xffff)})
        return rows
    raise RuntimeError("UDP ownership table kept changing size")


if __name__ == "__main__":
    import argparse
    import json
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pid", type=int)
    print(json.dumps(owned_udp_endpoints(parser.parse_args().pid)))
