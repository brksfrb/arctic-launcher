'use strict'
const { test } = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const camera = require('./camera.cjs')
const { loadProtocol } = require('./connector.cjs')
const { PacketRecording, PacketReplay, exportCameraReplay } = require('./replay.cjs')
const fixture = require('./fixtures/protocol.cjs')
const pose = (x, yaw = 0) => ({ x, y: 80, z: 0, yaw, pitch: 10, fov: 70 })

test('camera interpolation handles shortest yaw, holds, smooth easing and strict bounded paths', () => {
  const track = camera.cameraPath({ format: 1, keyframes: [
    { timeMs: 0, pose: pose(0, 170), ease: 'smooth' }, { timeMs: 1000, pose: pose(20, -170) }
  ] }, 1000)
  assert.equal(camera.sample(track, 500).yaw, 180)
  assert.equal(camera.sample(track, 250).x, 3.125)
  assert.equal(camera.sample(track, 1000).yaw, -170)
  assert.throws(() => camera.sample(track, -10), /Invalid camera sample time/)
  track.keyframes[0].ease = 'hold'
  assert.equal(camera.sample(track, 999).x, 0)
  assert.throws(() => camera.pose({ ...pose(1), x: Infinity }), /range/)
  assert.throws(() => camera.pose({ ...pose(1), pitch: 91 }), /range/)
  assert.throws(() => camera.cameraPath({ format: 1, keyframes: [
    { timeMs: 1, pose: pose(0) }, { timeMs: 1, pose: pose(1) }
  ] }), /increasing/)
  assert.throws(() => camera.cameraPath({ format: 1, keyframes: [{ timeMs: 1001, pose: pose(0) }] }, 1000), /within/)
})

test('camera protocol preserves preset indices and FOV easing is a wire string per Mojang 2193', () => {
  loadProtocol()
  const { createSerializer, createDeserializer } = require('bedrock-protocol/src/transforms/serializer')
  const serializer = createSerializer('1.26.51'), parser = createDeserializer('1.26.51')
  const base = camera.presets().presets[0], list = camera.presets([{ ...base, name: 'original:free' }])
  assert.equal(list.id, 1); assert.equal(list.presets[0].name, 'original:free')
  assert.equal(camera.presets(list.presets).id, 1)
  const bytes = serializer.createPacketBuffer({ name: 'camera_instruction', params: camera.instruction(pose(12, 45), list.id) })
  // Independent wire assertion: FOV begins with presence + two LE floats,
  // then length-prefixed UTF-8 easing + clear, followed by three absent options.
  const expectedTail = Buffer.alloc(1 + 8 + 7 + 1 + 3)
  expectedTail[0] = 1; expectedTail.writeFloatLE(70, 1); expectedTail.writeFloatLE(0, 5)
  Buffer.from('\x06linear').copy(expectedTail, 9)
  assert.ok(bytes.subarray(-expectedTail.length).equals(expectedTail), bytes.toString('hex'))
  const decoded = parser.parsePacketBuffer(bytes).data.params
  assert.equal(decoded.instruction_set.runtime_id, 1)
  assert.equal(decoded.instruction_set.rotation.z, 45)
  assert.equal(decoded.fov.ease_type, 'linear')
})

test('camera export keeps world packets and timestamps, bakes a bounded track, refuses overwrite', async () => {
  const bedrock = loadProtocol(), version = '1.26.51'
  const { createSerializer, createDeserializer } = require('bedrock-protocol/src/transforms/serializer')
  const serializer = createSerializer(version), parser = createDeserializer(version)
  assert.ok(bedrock.Server)
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'arctic-camera-export-'))
  const file = path.join(dir, 'world.abr'), output = path.join(dir, 'edited.abr')
  try {
    const recorder = new PacketRecording(file, { username: 'ArcticTest', version })
    const start = fixture(version, 'start_game'); start.runtime_entity_id = 1n
    const world = serializer.createPacketBuffer({ name: 'set_time', params: { time: 734 } })
    recorder.packetAt(0, 0, 'start_game', serializer.createPacketBuffer({ name: 'start_game', params: start }))
    recorder.packetAt(500, 0, 'set_time', world)
    await recorder.finish(1000)
    const track = { format: 1, keyframes: [{ timeMs: 0, pose: pose(0) }, { timeMs: 1000, pose: pose(20, 90) }] }
    await exportCameraReplay(file, output, track)
    const edited = new PacketReplay(output)
    try {
      assert.equal(edited.duration, 1000)
      assert.ok(edited.records.some(r => r.time === 500 && edited.packet(r).equals(world)))
      const packets = edited.records.map(r => ({ time: r.time, ...parser.parsePacketBuffer(edited.packet(r)).data }))
      assert.equal(packets[0].name, 'start_game')
      assert.equal(packets[1].name, 'camera_presets')
      const cameraPackets = packets.filter(p => p.name === 'camera_instruction')
      assert.equal(cameraPackets.length, 26)
      const middle = cameraPackets.find(p => p.time === 480).params
      assert.ok(Math.abs(middle.instruction_set.position.x - 9.6) < .00001)
      assert.equal(cameraPackets.at(-1).params.instruction_set.position.x, 20)
      assert.equal(cameraPackets.at(-1).params.fov.field_of_view, 70)
    } finally { edited.close() }
    await assert.rejects(exportCameraReplay(file, output, track), /EEXIST/)
    const intact = new PacketReplay(output)
    assert.equal(intact.duration, 1000); intact.close()
  } finally { fs.rmSync(dir, { recursive: true }) }
})

test('camera exports are byte-identical across wall clocks and sample 60 Hz without accumulated drift', async () => {
  loadProtocol()
  const { createSerializer, createDeserializer } = require('bedrock-protocol/src/transforms/serializer')
  const version = '1.26.51', serializer = createSerializer(version), parser = createDeserializer(version)
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'arctic-deterministic-export-'))
  const file = path.join(dir, 'world.abr'), a = path.join(dir, 'a.abr'), b = path.join(dir, 'b.abr')
  const track = { format: 1, keyframes: [{ timeMs: 0, pose: pose(0) }, { timeMs: 1000, pose: pose(20, 90) }] }
  try {
    const recording = new PacketRecording(file, { username: 'ArcticTest', version, createdAt: '2020-01-02T03:04:05.000Z' })
    recording.packetAt(0, 0, 'start_game', serializer.createPacketBuffer({ name: 'start_game', params: fixture(version, 'start_game') }))
    recording.packetAt(500, 0, 'set_time', serializer.createPacketBuffer({ name: 'set_time', params: { time: 734 } }))
    await recording.finish(1000)
    await exportCameraReplay(file, a, track, { fps: 60 })
    await new Promise(resolve => setTimeout(resolve, 25))
    await exportCameraReplay(file, b, track, { fps: 60 })
    assert.deepEqual(fs.readFileSync(a), fs.readFileSync(b))
    const edited = new PacketReplay(a)
    try {
      assert.equal(edited.metadata.createdAt, '2020-01-02T03:04:05.000Z')
      assert.deepEqual(edited.metadata.cameraExport, { format: 1, fps: 60 })
      const packets = edited.records.map(record => ({ time: record.time, ...parser.parsePacketBuffer(edited.packet(record)).data }))
      const samples = packets.filter(packet => packet.name === 'camera_instruction')
      assert.deepEqual(samples.map(packet => packet.time), Array.from({ length: 61 }, (_, i) => Math.floor(i * 1000 / 60)))
      const worldIndex = packets.findIndex(packet => packet.name === 'set_time')
      const cameraIndex = packets.findIndex(packet => packet.name === 'camera_instruction' && packet.time === 500)
      assert.ok(worldIndex < cameraIndex, 'world updates at a timestamp precede its camera sample')
      assert.equal(samples.at(-1).params.instruction_set.position.x, 20)
    } finally { edited.close() }
    for (const options of [{ fps: 0 }, { fps: 121 }, { fps: 29.97 }, { unexpected: true }, null]) {
      await assert.rejects(exportCameraReplay(file, path.join(dir, 'invalid.abr'), track, options), /options|FPS/)
      assert.ok(!fs.existsSync(path.join(dir, 'invalid.abr')))
    }
    const video = path.join(dir, 'wrong-format.mp4')
    await assert.rejects(exportCameraReplay(file, video, track), /absolute .abr/)
    assert.ok(!fs.existsSync(video), 'packet archives must never be mislabeled as video')
  } finally { fs.rmSync(dir, { recursive: true }) }
})

test('camera export decodes recipe items using the recorded shield registry', async () => {
  loadProtocol()
  const { createSerializer, createDeserializer } = require('bedrock-protocol/src/transforms/serializer')
  const version = '1.26.51', serializer = createSerializer(version)
  const shieldId = 731
  serializer.proto.setVariable('ShieldItemID', shieldId)
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'arctic-camera-item-registry-'))
  const file = path.join(dir, 'world.abr'), output = path.join(dir, 'edited.abr')
  try {
    const registry = serializer.createPacketBuffer({ name: 'item_registry', params: { itemstates: [{
      name: 'minecraft:shield', runtime_id: shieldId, component_based: false, version: 'none',
      nbt: { type: 'compound', name: '', value: {} }
    }] } })
    const recipes = fixture(version, 'crafting_data')
    recipes.shapeless_recipes = [{
      recipe_id: 'arctic:shield', input: [], output: [{
        network_id: shieldId, count: 1, metadata: 0, block_runtime_id: 0,
        extra: { has_nbt: 'false', can_place_on: [], can_destroy: [], blocking_tick: 123456789n }
      }], uuid: '00000000-0000-0000-0000-000000000001', block: 'crafting_table',
      priority: 0, unlocking_requirement: undefined, network_id: 1
    }]
    const crafting = serializer.createPacketBuffer({ name: 'crafting_data', params: recipes })
    const recording = new PacketRecording(file, { username: 'ArcticTest', version })
    recording.packetAt(0, 0, 'start_game', serializer.createPacketBuffer({ name: 'start_game', params: fixture(version, 'start_game') }))
    recording.packetAt(1, 0, 'item_registry', registry)
    recording.packetAt(2, 0, 'crafting_data', crafting)
    await recording.finish(100)
    await exportCameraReplay(file, output, { format: 1, keyframes: [{ timeMs: 0, pose: pose(0) }] })
    const edited = new PacketReplay(output)
    try {
      assert.ok(edited.records.some(r => r.time === 1 && edited.packet(r).equals(registry)))
      assert.ok(edited.records.some(r => r.time === 2 && edited.packet(r).equals(crafting)))
      const parser = createDeserializer(version)
      parser.proto.setVariable('ShieldItemID', shieldId)
      const decoded = parser.parsePacketBuffer(crafting).data.params
      assert.equal(decoded.shapeless_recipes[0].output[0].extra.blocking_tick.toString(), '123456789')
    } finally { edited.close() }
  } finally { fs.rmSync(dir, { recursive: true }) }
})
