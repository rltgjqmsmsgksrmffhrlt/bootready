// scripts/make-icon.js — Ring+Dot 아이콘 PNG 생성 (Node.js 내장만 사용)
const fs = require('fs')
const path = require('path')
const zlib = require('zlib')

const ACCENT = [0x1d, 0x9e, 0x75]

function drawIcon(size) {
  const pixels = new Uint8Array(size * size * 4) // RGBA

  const cx = (size - 1) / 2
  const R  = size * 0.40   // ring 반경
  const stroke = size * 0.11
  const dotR = size * 0.11
  const inner = R - stroke / 2
  const outer = R + stroke / 2

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const dx = x - cx, dy = y - cx
      const dist = Math.sqrt(dx * dx + dy * dy)
      let r = 0, g = 0, b = 0, a = 0

      // 중앙 점
      const dotA = Math.min(1, Math.max(0, dotR + 0.5 - dist))
      if (dotA > 0) {
        r = ACCENT[0]; g = ACCENT[1]; b = ACCENT[2]; a = dotA
      } else {
        // 링 밴드
        const bandA = Math.min(outer + 0.5 - dist, dist - inner + 0.5)
        const ba = Math.min(1, Math.max(0, bandA))
        if (ba > 0) {
          r = ACCENT[0]; g = ACCENT[1]; b = ACCENT[2]; a = ba
        }
      }

      const i = (y * size + x) * 4
      // pre-multiplied → straight
      pixels[i]   = r
      pixels[i+1] = g
      pixels[i+2] = b
      pixels[i+3] = Math.round(a * 255)
    }
  }
  return pixels
}

function crc32(buf) {
  let crc = 0xFFFFFFFF
  const table = crc32.table || (crc32.table = (() => {
    const t = new Uint32Array(256)
    for (let i = 0; i < 256; i++) {
      let c = i
      for (let j = 0; j < 8; j++) c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1)
      t[i] = c
    }
    return t
  })())
  for (let i = 0; i < buf.length; i++) crc = table[(crc ^ buf[i]) & 0xFF] ^ (crc >>> 8)
  return (crc ^ 0xFFFFFFFF) >>> 0
}

function chunk(type, data) {
  const t = Buffer.from(type, 'ascii')
  const d = Buffer.isBuffer(data) ? data : Buffer.from(data)
  const len = Buffer.alloc(4); len.writeUInt32BE(d.length)
  const crcBuf = Buffer.concat([t, d])
  const c = Buffer.alloc(4); c.writeUInt32BE(crc32(crcBuf))
  return Buffer.concat([len, t, d, c])
}

function makePNG(size) {
  const pixels = drawIcon(size)

  // Raw rows with filter byte 0
  const rows = Buffer.alloc(size * (1 + size * 4))
  for (let y = 0; y < size; y++) {
    rows[y * (1 + size * 4)] = 0
    for (let x = 0; x < size; x++) {
      const src = (y * size + x) * 4
      const dst = y * (1 + size * 4) + 1 + x * 4
      rows[dst]   = pixels[src]
      rows[dst+1] = pixels[src+1]
      rows[dst+2] = pixels[src+2]
      rows[dst+3] = pixels[src+3]
    }
  }

  const idat = zlib.deflateSync(rows, { level: 9 })

  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(size, 0)
  ihdr.writeUInt32BE(size, 4)
  ihdr[8] = 8; ihdr[9] = 6 // 8-bit RGBA

  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), // PNG sig
    chunk('IHDR', ihdr),
    chunk('IDAT', idat),
    chunk('IEND', Buffer.alloc(0)),
  ])
}

const outDir = path.join(__dirname, '../assets')
fs.mkdirSync(outDir, { recursive: true })

// 256px PNG (png-to-ico 입력용)
const png256 = makePNG(256)
fs.writeFileSync(path.join(outDir, 'icon-256.png'), png256)
console.log('icon-256.png written')

// 여러 사이즈도 저장 (참고용)
for (const sz of [16, 32, 48]) {
  fs.writeFileSync(path.join(outDir, `icon-${sz}.png`), makePNG(sz))
}
console.log('done')
