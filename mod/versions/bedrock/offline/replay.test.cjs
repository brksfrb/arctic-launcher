'use strict'
const { test } = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const dgram = require('node:dgram')
const { once } = require('node:events')
const { PacketRecording, PacketReplay, startPlayback, validatePlayback } = require('./replay.cjs')
const { loadProtocol, startConnector } = require('./connector.cjs')
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))
const fixture = require('./fixtures/protocol.cjs')

async function freePort() {
  const socket = dgram.createSocket('udp4'); socket.bind(0, '127.0.0.1'); await once(socket, 'listening')
  const { port } = socket.address(); await new Promise(resolve => socket.close(resolve)); return port
}
async function waitFor(predicate) {
  for (let n = 0; n < 500; n++) { if (predicate()) return; await sleep(10) }
  throw new Error('Timed out waiting for replay state')
}

test('packet recordings exclude authentication, seal, retain directions and refuse overwrites/truncation', async () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'arctic-replay-')), file = path.join(dir, 'test.abr')
  try {
    const recording = new PacketRecording(file, { username: 'ArcticTest', version: '1.26.51' })
    assert.throws(() => new PacketRecording(file, {}), /EEXIST/)
    recording.packet(0, 'server_to_client_handshake', Buffer.from('private-auth-token'))
    recording.packet(0, 'set_time', Buffer.from([10, 40]))
    recording.packet(1, 'move_player', Buffer.from([19, 20, 21]))
    await recording.finish()
    assert.equal(fs.readFileSync(file).includes('private-auth-token'), false)
    const replay = new PacketReplay(file)
    assert.equal(replay.records.length, 2)
    assert.equal(replay.records[1].direction, 1)
    assert.deepEqual(replay.packet(replay.records[0]), Buffer.from([10, 40])); replay.close()
    await assert.rejects(startPlayback({file}),/no world bootstrap/)
    const bad = path.join(dir, 'truncated.abr'); fs.writeFileSync(bad, fs.readFileSync(file).subarray(0, -1))
    assert.throws(() => new PacketReplay(bad), /truncated|finalized/)
    assert.throws(() => validatePlayback({ file, accessToken: 'no' }), /configuration/)
    assert.throws(() => validatePlayback({ file, speed: Infinity }), /speed/)
    assert.throws(() => validatePlayback({ file, startMs: -1 }), /start time/)
    for (const host of ['0.0.0.0', '192.0.2.1', 'localhost', '::1']) {
      assert.throws(() => validatePlayback({ file, host }), /loopback/)
    }
    assert.equal(validatePlayback({ file, host: '127.0.0.2' }).host, '127.0.0.2')
  } finally { fs.rmSync(dir, { recursive: true }) }
})

test('records real encrypted offline world packets and replays them on a fresh offline connection', { timeout: 30000 }, async () => {
  const bedrock = loadProtocol(), { CURRENT_VERSION: version } = require('bedrock-protocol/src/options')
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'arctic-replay-wire-')), file = path.join(dir, 'world.abr')
  const ports = new Set(); while (ports.size < 3) ports.add(await freePort())
  const [port, relayPort, replayPort] = [...ports]
  const server = new bedrock.Server({ host: '127.0.0.1', port, version, offline: true, raknetBackend: 'jsp-raknet' })
  let relay, player, client, playback, viewer
  const events = []
  try {
    const joined = new Promise(resolve => server.on('connect', p => {
      p.on('error', () => {}); p.once('join', () => resolve(p))
    }))
    await server.listen()
    relay = await startConnector({ host: '127.0.0.1', port, listenPort: relayPort,
      username: 'ArcticTest', version, recordTo: file }, e => events.push(e))
    client = bedrock.createClient({ host: '127.0.0.1', port: relayPort, version, username: 'ArcticTest',
      offline: true, autoInitPlayer: false, skipPing: true, raknetBackend: 'jsp-raknet', useRaknetWorkers: false, conLog: null })
    client.on('error', () => {}); player = await joined
    await waitFor(() => events.some(e => e.event === 'joined'))
    const packets = []
    const start = fixture(version, 'start_game')
    Object.assign(start, { entity_id: 1n, runtime_entity_id: 1n, generator: 1,
      player_position: { x: 1, y: 65, z: 1 }, world_name: 'Arctic offline replay test' })
    const block = fixture(version, 'update_block')
    Object.assign(block, { coordinates: { x: 3, y: 63, z: 4 }, block_runtime_id: 17 })
    for (const [name, params] of [['resource_packs_info',fixture(version,'resource_packs_info')],
      ['resource_pack_stack',fixture(version,'resource_pack_stack')],['start_game', start], ['set_time', { time: 1234 }], ['update_block', block],
      ['transfer',fixture(version,'transfer')]]) {
      if(name!=='transfer')packets.push(server.serializer.createPacketBuffer({ name, params }))
      player.queue(name, params); await sleep(25)
    }
    const movement=fixture(version,'player_auth_input')
    Object.assign(movement,{position:{x:8,y:66.6,z:9},yaw:45,pitch:12,head_yaw:45,tick:30n})
    client.queue('player_auth_input',movement)
    await sleep(100)
    await relay.close(); relay = null
    assert.ok(events.some(e => e.event === 'recording_saved'))
    const recorded = new PacketReplay(file)
    for (const bytes of packets) assert.ok(recorded.records.some(r => r.direction === 0 && recorded.packet(r).equals(bytes)))
    recorded.close()
    const playbackEvents = [], received = []
    playback = await startPlayback({ file, listenPort: replayPort, speed: 2 }, e => playbackEvents.push(e))
    viewer = bedrock.createClient({ host: '127.0.0.1', port: replayPort, version, username: 'ReplayGuest',
      offline: true, autoInitPlayer: false, skipPing: true, raknetBackend: 'jsp-raknet', useRaknetWorkers: false, conLog: null })
    viewer.on('error', () => {}); viewer.on('packet', p => received.push(p.fullBuffer))
    await waitFor(() => playbackEvents.some(e => e.event === 'replay_finished') || playbackEvents.some(e => e.event === 'error'))
    assert.equal(playbackEvents.some(e => e.event === 'error'), false, JSON.stringify(playbackEvents))
    await waitFor(() => packets.every(bytes => received.some(buffer => buffer.equals(bytes))))
    await waitFor(() => received.some(buffer=> {
      const p=playback.deserializer.parsePacketBuffer(buffer).data
      return p.name==='move_player' && p.params.position.x===8 && p.params.runtime_id===1
    }))
    assert.equal(received.some(buffer=>playback.deserializer.parsePacketBuffer(buffer).data.name==='transfer'),false)
    const replayPlayer = Object.values(playback.clients)[0]
    assert.equal(replayPlayer.authentication.authenticated, false)
    assert.equal(replayPlayer.authentication.method, 'offline')
    assert.equal(playback.options.host, '127.0.0.1')
    await playback.control({ op: 'pause', paused: true })
    await playback.control({ op: 'speed', speed: .5 })
    await playback.control({ op: 'camera', pose: { x: 12, y: 80, z: 15, yaw: 45, pitch: 20, fov: 90 } })
    await waitFor(() => received.some(buffer => {
      const p = playback.deserializer.parsePacketBuffer(buffer).data
      return p.name === 'camera_instruction' && p.params.instruction_set?.position.x === 12 && p.params.fov?.field_of_view === 90
    }))
    const state = playbackEvents.findLast(e => e.event === 'replay_state')
    assert.equal(state.paused, true); assert.equal(state.speed, .5); assert.equal(state.camera, 'fixed')
    await assert.rejects(playback.control({ op: 'seek', timeMs: 0 }), /Backward seek/)
    await assert.rejects(playback.control({ op: 'speed', speed: NaN }), /speed/)
    await assert.rejects(playback.control({ op: 'pause', paused: 'yes' }), /boolean/)
    await playback.control({ op: 'camera_clear' })
    await waitFor(() => received.some(buffer => {
      const p = playback.deserializer.parsePacketBuffer(buffer).data
      return p.name === 'camera_instruction' && p.params.clear === true && p.params.fov.clear === true
    }))
  } finally {
    client?.close(); viewer?.close(); await sleep(100)
    await relay?.close(); await playback?.close()
    for (const p of Object.values(server.clients)) p.close('test finished')
    await server.close(); await sleep(100)
    fs.rmSync(dir, { recursive: true })
  }
})

test('paused playback still bootstraps, forward seek restores updates in order, and resume advances the clock', { timeout: 30000 }, async () => {
  const bedrock = loadProtocol(), version = '1.26.51'
  const { createSerializer } = require('bedrock-protocol/src/transforms/serializer')
  const serializer = createSerializer(version)
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'arctic-replay-clock-')), file = path.join(dir, 'clock.abr')
  let playback, viewer
  try {
    const recording = new PacketRecording(file, { username: 'ArcticTest', version })
    const start = fixture(version, 'start_game'); start.runtime_entity_id = 1n
    for (const [time, name, params] of [[0, 'resource_packs_info', fixture(version, 'resource_packs_info')],
      [0, 'resource_pack_stack', fixture(version, 'resource_pack_stack')], [0, 'start_game', start],
      [1000, 'set_time', { time: 111 }], [1500, 'set_time', { time: 222 }], [2500, 'set_time', { time: 333 }]]) {
      recording.packetAt(time, 0, name, serializer.createPacketBuffer({ name, params }))
    }
    await recording.finish(3000)
    const events = [], times = []
    playback = await startPlayback({ file, listenPort: await freePort() }, event => events.push(event))
    await playback.control({ op: 'pause', paused: true })
    viewer = bedrock.createClient({ host: '127.0.0.1', port: playback.options.port, version, username: 'ArcticTest',
      offline: true, autoInitPlayer: false, skipPing: true, raknetBackend: 'jsp-raknet', useRaknetWorkers: false, conLog: null })
    viewer.on('error', () => {}); viewer.on('set_time', p => times.push(p.time))
    await waitFor(() => viewer.startGameData.runtime_entity_id != null)
    await sleep(700)
    assert.deepEqual(times, [])
    await playback.control({ op: 'seek', timeMs: 1500 })
    await waitFor(() => times.length === 2)
    assert.deepEqual(times, [111, 222])
    await sleep(300)
    assert.equal(events.findLast(e => e.event === 'replay_state').positionMs, 1500)
    assert.deepEqual(times, [111, 222])
    await assert.rejects(playback.control({ op: 'seek', timeMs: 500 }), /Backward seek/)
    await playback.control({ op: 'speed', speed: 8 })
    await playback.control({ op: 'pause', paused: false })
    await waitFor(() => events.some(e => e.event === 'replay_finished'))
    assert.deepEqual(times, [111, 222, 333])
  } finally {
    viewer?.close(); await sleep(100); await playback?.close(); await sleep(100)
    fs.rmSync(dir, { recursive: true })
  }
})

test('exact packet steps drain every batch, hold the camera, and reset through a fresh encrypted connection', { timeout: 30000 }, async () => {
  const bedrock = loadProtocol(), version = '1.26.51'
  const { createSerializer } = require('bedrock-protocol/src/transforms/serializer')
  const serializer = createSerializer(version)
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'arctic-replay-step-')), file = path.join(dir, 'step.abr')
  let playback, first, second, third
  try {
    const recording = new PacketRecording(file, { username: 'ArcticTest', version })
    const start = fixture(version, 'start_game'); start.runtime_entity_id = 1n
    const packet = (time, name, params) => recording.packetAt(time, 0, name, serializer.createPacketBuffer({ name, params }))
    packet(0, 'resource_packs_info', fixture(version, 'resource_packs_info'))
    packet(0, 'resource_pack_stack', fixture(version, 'resource_pack_stack'))
    packet(0, 'start_game', start)
    // More than two send batches, all at the target timestamp. An acknowledgement
    // after only the first 128 packets would miss most of this world's updates.
    for (let i = 0; i < 300; i++) packet(1000, 'set_time', { time: i })
    packet(1500, 'set_time', { time: 999 })
    await recording.finish(3000)
    const events = [], times = [[], [], []], cameras = [[], [], []]
    playback = await startPlayback({ file, listenPort: await freePort() }, event => events.push(event))
    await assert.rejects(playback.control({ op: 'step', generation: 1, sequence: 1, timeMs: 1000 }), /paused playback/)
    await playback.control({ op: 'pause', paused: true })
    await assert.rejects(playback.control({ op: 'step', generation: 1, sequence: 1, timeMs: 1000 }), /game connection/)
    await playback.control({ op: 'camera_path', path: { format: 1, keyframes: [
      { timeMs: 0, pose: { x: 0, y: 80, z: 0, yaw: 0, pitch: 0, fov: 70 } },
      { timeMs: 2000, pose: { x: 20, y: 80, z: 0, yaw: 0, pitch: 0, fov: 70 } }
    ] } })
    const connect = index => {
      const client = bedrock.createClient({ host: '127.0.0.1', port: playback.options.port, version, username: 'ArcticTest',
        offline: true, autoInitPlayer: false, skipPing: true, raknetBackend: 'jsp-raknet', useRaknetWorkers: false, conLog: null })
      client.on('error', () => {})
      client.on('set_time', p => times[index].push(p.time))
      client.on('camera_instruction', p => { if (p.instruction_set) cameras[index].push(p.instruction_set.position.x) })
      return client
    }
    first = connect(0)
    await waitFor(() => first.startGameData.runtime_entity_id != null)
    const result = await playback.control({ op: 'step', generation: 1, sequence: 1, timeMs: 1000 })
    assert.deepEqual(result, { event: 'replay_step', generation: 1, sequence: 1, positionMs: 1000,
      packetsQueued: true, nativeWorldVerified: false, xboxAuth: false })
    await waitFor(() => times[0].length === 300 && cameras[0].at(-1) === 10)
    assert.deepEqual(times[0], Array.from({ length: 300 }, (_, i) => i))
    const retiredPeer = Object.values(playback.clients)[0].connection
    const count = cameras[0].length
    await sleep(150)
    assert.equal(cameras[0].length, count, 'paused camera must not be resent on a wall-clock interval')
    let state = await playback.control({ op: 'state' })
    assert.equal(state.packetReady, true); assert.equal(state.positionMs, 1000); assert.equal(state.pendingStep, null)
    await assert.rejects(playback.control({ op: 'step', generation: 1, sequence: 1, timeMs: 1000 }), /sequence/)
    await assert.rejects(playback.control({ op: 'step', generation: 2, sequence: 2, timeMs: 1000 }), /generation/)
    await assert.rejects(playback.control({ op: 'step', generation: 1, sequence: 2, timeMs: 999 }), /Backward step/)
    await assert.rejects(playback.control({ op: 'step', generation: 1, sequence: 2, timeMs: 1000, accessToken: 'no' }), /Invalid replay control/)
    const pending = playback.control({ op: 'step', generation: 1, sequence: 2, timeMs: 2000 })
    const canceled = assert.rejects(pending, /canceled by world reset/)
    await assert.rejects(playback.control({ op: 'pause', paused: false }), /pending replay step/)
    await assert.rejects(playback.control({ op: 'reset', timeMs: null }), /Reset time/)
    assert.equal((await playback.control({ op: 'state' })).pendingStep.sequence, 2)
    state = await playback.control({ op: 'reset', timeMs: 500 })
    await canceled
    assert.equal(state.generation, 2); assert.equal(state.positionMs, 500)
    assert.equal(state.paused, true); assert.equal(state.connectionRequired, true); assert.equal(state.packetReady, false)
    assert.equal(events.some(e => e.event === 'replay_step' && e.sequence === 2), false)
    await waitFor(() => Object.keys(playback.clients).length === 0)
    second = connect(1)
    await waitFor(() => second.startGameData.runtime_entity_id != null)
    await waitFor(() => events.some(e => e.event === 'replay_state' && e.generation === 2 && e.packetReady))
    assert.deepEqual(times[1], [], 'the rebuilt world must not retain updates from after its reset target')
    await assert.rejects(playback.control({ op: 'step', generation: 1, sequence: 1, timeMs: 1000 }), /generation/)
    await playback.control({ op: 'step', generation: 2, sequence: 1, timeMs: 1000 })
    await waitFor(() => times[1].length === 300 && cameras[1].at(-1) === 10)
    assert.deepEqual(times[1], times[0], 'both generations must deliver the same recorded world updates')
    assert.deepEqual(second.accessToken, [])
    const player = Object.values(playback.clients)[0]
    assert.equal(player.authentication.method, 'offline'); assert.equal(player.authentication.authenticated, false)
    retiredPeer.disconnect('late close from the previous replay generation')
    assert.equal(playback.clientCount, 1)
    assert.equal(playback.clients[player.connection.address], player)
    assert.equal(playback.transport.raknet.connections.get(player.connection.address.hash), player.connection)
    second.close()
    await waitFor(() => playback.clientCount === 0)
    assert.equal(playback.transport.raknet.running, true, 'one viewer disconnect must not close the replay server')
    third = connect(2)
    await waitFor(() => times[2].length === 300)
    assert.deepEqual(times[2], times[0])
    const stopped = assert.rejects(playback.control({ op: 'step', generation: 2, sequence: 2, timeMs: 1500 }), /Replay stopped/)
    await playback.close(); playback = null; await stopped
  } finally {
    first?.close(); second?.close(); third?.close(); await sleep(100); await playback?.close(); await sleep(100)
    assert.equal(path.dirname(dir), os.tmpdir()); assert.ok(path.basename(dir).startsWith('arctic-replay-step-'))
    fs.rmSync(dir, { recursive: true })
  }
})
