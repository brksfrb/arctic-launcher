// bots.js <port> <count> <version> [protocolOverride]: offline bot players that log in, stay, swing and chat now and then.
const mc = require('/home/claude/joinpass-work/npm/node_modules/minecraft-protocol');
const [port, count, version, proto] = [Number(process.argv[2]), Number(process.argv[3]), process.argv[4], process.argv[5]];
let joined = 0, failed = 0;
function start(i) {
  const name = 'Bot_' + String(i).padStart(3, '0');
  const opts = { host: '127.0.0.1', port, username: name, version, auth: 'offline', hideErrors: true };
  if (proto) opts.protocolVersion = Number(proto);
  let c;
  try { c = mc.createClient(opts); } catch (e) { console.log('create failed', name, e.message); failed++; return; }
  if (proto) {
    // Talk 26.1's packets while saying 26.3's protocol number in the handshake.
    const w = c.write.bind(c);
    c.write = (name, params) => { if (name === 'set_protocol') params = Object.assign({}, params, { protocolVersion: Number(proto) }); return w(name, params); };
  }
  let timer = null;
  c.on('login', () => {
    joined++;
    if (joined === count) console.log('BOTS STARTED ' + joined);
    timer = setInterval(() => { try { c.write('arm_animation', { hand: 0 }); } catch (e) {} }, 700 + (i * 37) % 600);
  });
  // A server resource pack: bots say no (players accept it; see the client side).
  c.on('add_resource_pack', (p) => { try { c.write('resource_pack_receive', { uuid: p.uuid, result: 1 }); } catch (e) { console.log('pack reply', e.message); } });
  c.on('position', (p) => { try { c.write('teleport_confirm', { teleportId: p.teleportId }); } catch (e) {} });
  c.on('error', (e) => { console.log('error', name, e.message); });
  c.on('end', (r) => { if (timer) clearInterval(timer); console.log('end', name, String(r).slice(0, 120)); });
}
for (let i = 0; i < count; i++) setTimeout(() => start(i), i * 120);
setInterval(() => console.log('joined', joined, 'failed', failed), 15000);
