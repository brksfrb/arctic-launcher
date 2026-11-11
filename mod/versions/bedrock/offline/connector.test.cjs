'use strict'

const { test } = require('node:test')
const assert = require('node:assert/strict')
const dgram = require('node:dgram')
const { once } = require('node:events')
const { validateConfig, startConnector, loadProtocol, offlineUuid } = require('./connector.cjs')

test('offline configuration rejects online-auth options and invalid identities', () => {
  assert.throws(() => validateConfig({ host: 'localhost', username: 'Alex', offline: false }), /Unsupported/)
  assert.throws(() => validateConfig({ host: 'localhost', username: 'Alex', realms: {} }), /Unsupported/)
  assert.throws(() => validateConfig({ host: 'localhost', username: '../Alex' }), /offline name/)
  assert.throws(() => validateConfig({ host: '127.0.0.1', port: 19142, username: 'Alex' }), /itself/)
  assert.throws(() => validateConfig({ host: 'https://example.com', username: 'Alex' }), /hostname/)
  assert.deepEqual(validateConfig({ host: 'localhost', username: 'Alex' }), {
    host: 'localhost', username: 'Alex', port: 19132, listenPort: 19142, version: null
  })
  assert.equal(offlineUuid('Notch'), 'b50ad385-829d-3141-a216-7e7d7539ba7f')
  assert.throws(() => validateConfig({ host: 'localhost', username: 'Alex', appearance: { accessToken: 'no' } }), /appearance/)
  assert.throws(() => validateConfig({ host: 'localhost', username: 'Alex', appearance: { slim: true } }), /skin/)
  assert.throws(() => validateConfig({ host: 'localhost', username: 'Alex', appearance: {
    skin: { width: 64, height: 64, data: 'AA==' }
  } }), /buffer/)
})

async function freePort() {
  const socket = dgram.createSocket('udp4')
  socket.bind(0, '127.0.0.1')
  await once(socket, 'listening')
  const { port } = socket.address()
  await new Promise(resolve => socket.close(resolve))
  return port
}

for (const custom of [true, false]) test(`real encrypted ${custom ? 'modern/custom skin and cape' : 'legacy/passthrough skin'} login uses an offline identity`, { timeout: 25000 }, async () => {
  const bedrock = loadProtocol()
  const { CURRENT_VERSION } = require('bedrock-protocol/src/options')
  const version = custom ? CURRENT_VERSION : '1.21.130'
  const skin = { width: 64, height: 64, data: Buffer.alloc(64 * 64 * 4, Buffer.from([13, 27, 191, 255])).toString('base64') }
  const cape = { width: 64, height: 32, data: Buffer.alloc(64 * 32 * 4, Buffer.from([71, 53, 29, 255])).toString('base64') }
  const port = await freePort()
  let listenPort = await freePort()
  while (listenPort === port) listenPort = await freePort()
  const server = new bedrock.Server({ host: '127.0.0.1', port, version,
    offline: true, raknetBackend: 'jsp-raknet' })
  let relay, client
  try {
    await server.listen()
    const backendJoin = new Promise((resolve, reject) => {
      server.on('connect', player => {
        player.on('error', reject)
        player.on('join', () => resolve(player))
      })
    })
    const events = []
    relay = await startConnector({ host: '127.0.0.1', port, listenPort, username: 'ArcticGuest', version,
      ...(custom ? { appearance: { skin, cape, slim: true } } : {}) },
      event => events.push(event))
    const ready = events.find(e => e.event === 'ready')
    assert.equal(ready.xboxAuth, false)
    assert.equal(relay.options.host, '127.0.0.1')
    client = bedrock.createClient({ host: '127.0.0.1', port: listenPort, version,
      username: 'OriginalLocalName', offline: true, skipPing: true, raknetBackend: 'jsp-raknet',
      useRaknetWorkers: false, conLog: null })
    client.on('error', () => {})
    const player = await backendJoin
    assert.equal(player.profile.name, 'ArcticGuest')
    assert.equal(player.profile.uuid, offlineUuid('ArcticGuest'))
    assert.ok([undefined, null, '', 0, '0'].includes(player.profile.xuid))
    assert.equal(player.authentication.authenticated, false)
    assert.equal(player.authentication.method, 'offline')
    const upstream = [...relay.upstreams.values()][0]
    assert.equal(upstream.options.offline, true)
    if (custom) {
      const identity = JSON.parse(Buffer.from(upstream.multiplayerToken.split('.')[1], 'base64url').toString('utf8'))
      assert.equal(identity.iss, 'self')
      assert.equal(identity.xid, '0')
      assert.equal(identity.xname, 'ArcticGuest')
      assert.equal(player.skinData.SkinData, skin.data)
      assert.equal(player.skinData.CapeData, cape.data)
      assert.equal(player.skinData.SkinImageWidth, 64)
      assert.equal(player.skinData.CapeImageHeight, 32)
      assert.equal(player.skinData.PersonaSkin, false)
      assert.equal(player.skinData.TrustedSkin, false)
      assert.equal(player.skinData.ArmSize, 'slim')
      assert.equal(JSON.parse(Buffer.from(player.skinData.SkinResourcePatch, 'base64')).geometry.default,
        'geometry.humanoid.customSlim')
    } else {
      assert.equal(player.skinData.SkinData, client.options.skinData?.SkinData
        ?? require('minecraft-data')(`bedrock_${version}`).defaultSkin.SkinData)
    }
    assert.deepEqual(upstream.accessToken, [])
    // An online auth request is a hard error, never a cached-token fallback.
    assert.throws(() => require('bedrock-protocol/src/client/auth').authenticate(), /disabled/)
  } finally {
    client?.close()
    // jsp-raknet drains final datagrams asynchronously when a peer closes.
    await new Promise(resolve => setTimeout(resolve, 150))
    await relay?.close()
    await new Promise(resolve => setTimeout(resolve, 150))
    for (const player of Object.values(server.clients)) player.close('test finished')
    await server.close()
    await new Promise(resolve => setTimeout(resolve, 150))
  }
})
