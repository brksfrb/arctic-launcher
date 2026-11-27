"""mcpr.py <file.mcpr> [--head N]: what a saved replay holds. Packet count and bytes, how many packets sit at the
first timestamp (the "opening" the viewer applies in one frame), and a histogram by packet id for the play phase
(names for protocol 47 = 1.8.x only; other versions print ids)."""
import sys, zipfile, struct, json, collections
P47 = {0x00: 'keep_alive', 0x01: 'login', 0x02: 'chat', 0x03: 'set_time', 0x04: 'set_equipment', 0x05: 'spawn_position', 0x06: 'set_health',
       0x07: 'respawn', 0x08: 'player_position', 0x09: 'held_slot', 0x0B: 'animate', 0x0C: 'add_player', 0x0D: 'take_item', 0x0E: 'add_entity',
       0x0F: 'add_mob', 0x10: 'add_painting', 0x11: 'add_xp_orb', 0x12: 'set_entity_motion', 0x13: 'remove_entities', 0x14: 'entity',
       0x15: 'move_entity_pos', 0x16: 'move_entity_rot', 0x17: 'move_entity_pos_rot', 0x18: 'teleport_entity', 0x19: 'rotate_head',
       0x1A: 'entity_event', 0x1B: 'set_entity_link', 0x1C: 'set_entity_data', 0x1D: 'mob_effect', 0x1F: 'set_xp', 0x20: 'update_attributes',
       0x21: 'level_chunk', 0x22: 'chunk_blocks_update', 0x23: 'block_update', 0x24: 'block_event', 0x25: 'block_destruction',
       0x26: 'map_chunk_bulk', 0x27: 'explode', 0x28: 'level_event', 0x29: 'sound', 0x2A: 'particles', 0x2B: 'game_event', 0x2F: 'set_slot',
       0x30: 'window_items', 0x33: 'sign', 0x35: 'block_entity_data', 0x37: 'stats', 0x38: 'player_info', 0x39: 'abilities', 0x3B: 'objective',
       0x3C: 'score', 0x3D: 'display_objective', 0x3E: 'team', 0x3F: 'custom_payload', 0x41: 'difficulty', 0x44: 'world_border', 0x45: 'title',
       0x47: 'tab_header'}
def varint(b, i):
    v = s = 0
    while True:
        k = b[i]; i += 1; v |= (k & 127) << s; s += 7
        if k < 128: return v, i
z = zipfile.ZipFile(sys.argv[1]); meta = json.loads(z.read('metaData.json'))
data = z.read('recording.tmcpr'); head = int(sys.argv[sys.argv.index('--head') + 1]) if '--head' in sys.argv else 0
i = n = 0; first = None; opening = obytes = 0; hist = collections.Counter(); hbytes = collections.Counter(); ohist = collections.Counter()
login = True; names = P47 if meta.get('protocol') == 47 else {}
while i + 8 <= len(data):
    t, ln = struct.unpack('>ii', data[i:i + 8]); pid, _ = varint(data, i + 8)
    if first is None: first = t
    key = ('login:%d' % pid) if login else names.get(pid, hex(pid))
    if head and n < head: print(f'   #{n} t={t} len={ln} {key}')
    if t == first: opening += 1; obytes += ln; ohist[key] += 1
    hist[key] += 1; hbytes[key] += ln
    if login and pid == 2 and meta.get('protocol') == 47: login = False
    elif login and meta.get('protocol') != 47 and n > 40: login = False
    i += 8 + ln; n += 1
print(f'{sys.argv[1].split("/")[-1]}: mc {meta.get("mcversion")} duration {meta.get("duration")} ms, {n} packets, {len(data) / 1e6:.1f} MB raw, zip {sum(e.compress_size for e in z.infolist()) / 1e6:.1f} MB')
print(f'  opening (t={first}): {opening} packets, {obytes / 1e6:.1f} MB = {100.0 * opening / max(1, n):.0f}% of packets')
print('  opening by kind: ' + ', '.join(f'{k} {v}' for k, v in ohist.most_common(8)))
print('  all by kind: ' + ', '.join(f'{k} {v} ({hbytes[k] / 1e3:.0f} KB)' for k, v in hist.most_common(10)))
