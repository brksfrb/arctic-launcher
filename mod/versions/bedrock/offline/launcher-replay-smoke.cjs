'use strict'
// Real built-CLI/helper lifecycle and encrypted wire playback; no game or login UI.
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { spawn } = require('node:child_process')
const dgram = require('node:dgram')
const { once } = require('node:events')
const { loadProtocol, offlineUuid } = require('./connector.cjs')
const { PacketReplay } = require('./replay.cjs')
const fixture = require('./fixtures/protocol.cjs')
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))

async function freePort() {
  const socket = dgram.createSocket('udp4'); socket.bind(0, '127.0.0.1'); await once(socket, 'listening')
  const { port } = socket.address(); await new Promise(resolve => socket.close(resolve)); return port
}
async function until(predicate, message, timeout = 10000) {
  const start = performance.now()
  while (!predicate()) {
    if (performance.now() - start > timeout) throw new Error(message())
    await sleep(20)
  }
}
function cli(exe, data, args) {
  const child = spawn(exe, ['--data-dir', data, '--json', 'bedrock', ...args], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] })
  child.events = []; child.diagnostics = ''; let pending = ''
  child.on('error', error => { child.spawnError = error })
  child.stderr.on('data', data => { child.diagnostics += data })
  child.stdout.on('data', data => {
    child.diagnostics += data; pending += data
    let end
    while ((end = pending.indexOf('\n')) >= 0) {
      const line = pending.slice(0, end); pending = pending.slice(end + 1)
      try { child.events.push(JSON.parse(line)) } catch {}
    }
  })
  return child
}
async function ready(child, event) {
  await until(() => child.events.some(e => e.event === event) || child.exitCode != null || child.signalCode != null || child.spawnError,
    () => `CLI startup timeout: ${child.diagnostics}`, 110000)
  if (child.spawnError) throw child.spawnError
  const result = child.events.find(e => e.event === event)
  assert.ok(result, `CLI exited without ${event}: ${child.diagnostics}`)
  assert.equal(result.xbox_auth, false)
  return result
}
async function stop(child) {
  if (!child || child.exitCode != null || child.signalCode != null) return
  child.stdin.write('stop\n')
  await until(() => child.exitCode != null || child.signalCode != null, () => `CLI failed to stop: ${child.diagnostics}`, 10000)
  assert.equal(child.exitCode, 0, child.diagnostics)
}

async function main() {
  const exe = path.resolve(process.argv[2]), data = path.resolve(process.argv[3])
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'arctic-replay-cli-')), file = path.join(dir, 'world.abr')
  const protocol = loadProtocol(), { CURRENT_VERSION: version } = require('bedrock-protocol/src/options')
  const ports = new Set(); while (ports.size < 3) ports.add(await freePort())
  const [serverPort, relayPort, replayPort] = [...ports]
  const server = new protocol.Server({ host: '127.0.0.1', port: serverPort, version, offline: true, raknetBackend: 'jsp-raknet' })
  let recorder, playback, client, viewer
  try {
    const joined = new Promise(resolve => server.on('connect', p => { p.on('error', () => {}); p.once('join', () => resolve(p)) }))
    await server.listen()
    recorder = cli(exe, data, ['offline', `127.0.0.1:${serverPort}`, '--name', 'ArcticTest', '--record-to', file,
      '--listen-port', String(relayPort), '--version', version])
    await ready(recorder, 'offline_ready')
    client = protocol.createClient({ host: '127.0.0.1', port: relayPort, version, username: 'ArcticTest',
      offline: true, autoInitPlayer: false, skipPing: true, raknetBackend: 'jsp-raknet', useRaknetWorkers: false, conLog: null })
    client.on('error', () => {})
    const player = await joined
    assert.equal(player.profile.uuid, offlineUuid('ArcticTest')); assert.equal(player.authentication.authenticated, false)
    await until(() => recorder.events.some(e => e.event === 'joined'), () => recorder.diagnostics)
    let seenTime = false
    client.on('set_time', p => { if (p.time === 4321) seenTime = true })
    const start = fixture(version, 'start_game')
    Object.assign(start, { entity_id: 1n, runtime_entity_id: 1n, generator: 1,
      player_position: { x: 0, y: 65, z: 0 }, world_name: 'CLI offline replay' })
    for (const [name, params] of [['resource_packs_info', fixture(version, 'resource_packs_info')],
      ['resource_pack_stack', fixture(version, 'resource_pack_stack')], ['start_game', start], ['set_time', { time: 4321 }]]) {
      player.queue(name, params); await sleep(30)
    }
    await until(() => seenTime, () => `Relay sent no world packet: ${recorder.diagnostics}`)
    if(process.argv[4]==='--kill-owner') {
      const exited=once(recorder,'exit');recorder.kill();await exited
      await until(()=> {
        try {const check=new PacketReplay(file);check.close();return true} catch {return false}
      },()=>`Recording did not finalize after its owner exited: ${recorder.diagnostics}`,10000)
    } else {
      await stop(recorder)
      assert.ok(recorder.events.some(e => e.event === 'recording_saved'), recorder.diagnostics)
    }
    const saved = new PacketReplay(file); assert.equal(saved.metadata.username, 'ArcticTest'); const duration = saved.duration; saved.close()
    playback = cli(exe, data, ['replay', file, '--listen-port', String(replayPort), '--speed', '2'])
    await ready(playback, 'replay_ready')
    let replayed = false
    viewer = protocol.createClient({ host: '127.0.0.1', port: replayPort, version, username: 'ReplayGuest',
      offline: true, autoInitPlayer: false, skipPing: true, raknetBackend: 'jsp-raknet', useRaknetWorkers: false, conLog: null })
    viewer.on('error', () => {}); viewer.on('set_time', p => { if (p.time === 4321) replayed = true })
    await until(() => replayed, () => `Replay sent no recorded world: ${playback.diagnostics}`)
    assert.equal(viewer.startGameData.world_name, 'CLI offline replay')
    assert.deepEqual(viewer.accessToken, [])
    await until(() => playback.events.some(e => e.event === 'replay_finished'), () => playback.diagnostics)
    playback.stdin.write('pause\n')
    await until(() => playback.events.some(e => e.event === 'replay_state' && e.paused), () => playback.diagnostics)
    let cameraSeen = false
    viewer.on('camera_instruction', p => { if (p.instruction_set?.position.x === 12 && p.fov?.field_of_view === 90) cameraSeen = true })
    playback.stdin.write(JSON.stringify({ op: 'camera', pose: { x: 12, y: 80, z: 15, yaw: 45, pitch: 20, fov: 90 } }) + '\n')
    await until(() => cameraSeen, () => `CLI camera command did not reach its viewer: ${playback.diagnostics}`)
    const edited = path.join(dir, 'edited.abr')
    const cameraPath = { format: 1, keyframes: [
      { timeMs: 0, pose: { x: 0, y: 80, z: 0, yaw: 0, pitch: 0, fov: 70 } },
      { timeMs: duration, pose: { x: 25, y: 90, z: 30, yaw: 90, pitch: 10, fov: 90 } }
    ] }
    playback.stdin.write(JSON.stringify({ op: 'export', file: edited, path: cameraPath, fps: 60 }) + '\n')
    await until(() => playback.events.some(e => e.event === 'replay_exported'), () => `CLI camera export failed: ${playback.diagnostics}`)
    const exported = new PacketReplay(edited)
    assert.deepEqual(exported.metadata.cameraExport, { format: 1, fps: 60 }); exported.close()
    const identical = path.join(dir, 'identical.abr')
    playback.stdin.write(JSON.stringify({ op: 'export', file: identical, path: cameraPath, fps: 60 }) + '\n')
    await until(() => playback.events.filter(e => e.event === 'replay_exported').length === 2,
      () => `CLI deterministic re-export failed: ${playback.diagnostics}`)
    assert.deepEqual(fs.readFileSync(edited), fs.readFileSync(identical))
    playback.stdin.write(JSON.stringify({ op: 'reset', timeMs: 0 }) + '\n')
    await until(() => playback.events.some(e => e.event === 'replay_reset' && e.generation === 2 && e.connection_required),
      () => `CLI reset acknowledgement lost its generation: ${playback.diagnostics}`)
    await until(() => playback.events.some(e => e.event === 'replay_disconnected' && e.generation === 1),
      () => `CLI reset did not retire the old connection: ${playback.diagnostics}`)
    viewer.close()
    let rebuiltWorld = false
    viewer = protocol.createClient({ host: '127.0.0.1', port: replayPort, version, username: 'ArcticTest',
      offline: true, autoInitPlayer: false, skipPing: true, raknetBackend: 'jsp-raknet', useRaknetWorkers: false, conLog: null })
    viewer.on('error', () => {}); viewer.on('set_time', p => { if (p.time === 4321) rebuiltWorld = true })
    await until(() => playback.events.some(e => e.event === 'replay_state' && e.generation === 2 && e.packet_ready),
      () => `CLI rebuilt connection never reached its packet bootstrap: ${playback.diagnostics}`)
    assert.equal(rebuiltWorld, false, 'reset must not keep the previous world updates')
    playback.stdin.write(JSON.stringify({ op: 'step', generation: 2, sequence: 1, timeMs: duration }) + '\n')
    await until(() => playback.events.some(e => e.event === 'replay_step' && e.generation === 2 && e.sequence === 1),
      () => `CLI exact packet step was not acknowledged: ${playback.diagnostics}`)
    const step = playback.events.find(e => e.event === 'replay_step' && e.generation === 2 && e.sequence === 1)
    assert.equal(step.position_ms, duration); assert.equal(step.packets_queued, true); assert.equal(step.native_world_verified, false)
    await until(() => rebuiltWorld, () => `CLI stepped replay lost its recorded world update: ${playback.diagnostics}`)
    playback.stdin.write(JSON.stringify({ op: 'step', generation: 2, sequence: 1, timeMs: duration }) + '\n')
    await until(() => playback.events.some(e => e.event === 'control_error' && e.generation === 2 && e.sequence === 1),
      () => `CLI stale step error lost its correlation: ${playback.diagnostics}`)
    await stop(playback)
    viewer.close(); await sleep(150)
    playback = cli(exe, data, ['replay', edited, '--listen-port', String(replayPort), '--speed', '2'])
    await ready(playback, 'replay_ready')
    let exportedCamera = false, exportedWorld = false
    viewer = protocol.createClient({ host: '127.0.0.1', port: replayPort, version, username: 'ReplayGuest',
      offline: true, autoInitPlayer: false, skipPing: true, raknetBackend: 'jsp-raknet', useRaknetWorkers: false, conLog: null })
    viewer.on('error', () => {})
    viewer.on('set_time', p => { if (p.time === 4321) exportedWorld = true })
    viewer.on('camera_instruction', p => { if (p.instruction_set?.position.x === 25 && p.fov?.field_of_view === 90) exportedCamera = true })
    await until(() => exportedWorld && exportedCamera, () => `Edited CLI replay lost its world/camera: ${playback.diagnostics}`)
    assert.deepEqual(viewer.accessToken, [])
    await stop(playback)
    console.log('Built Rust CLI / offline recording / camera export / reset generation / exact packet step and stale error / fresh encrypted replay playback / graceful shutdown passed')
  } finally {
    client?.close(); viewer?.close(); await sleep(150)
    for (const child of [recorder, playback]) if (child && child.exitCode == null && child.signalCode == null) {
      try { await stop(child) } catch { const exited = once(child, 'exit'); child.kill(); await exited }
    }
    for (const p of Object.values(server.clients)) p.close('test finished')
    await server.close(); await sleep(150)
    assert.equal(path.dirname(dir), os.tmpdir())
    assert.ok(path.basename(dir).startsWith('arctic-replay-cli-'))
    fs.rmSync(dir, { recursive: true })
  }
}
const watchdog = setTimeout(() => { console.error('Replay CLI smoke timeout'); process.exit(1) }, 150000)
main().then(() => clearTimeout(watchdog), error => { console.error(error); process.exitCode = 1; clearTimeout(watchdog) })
