// ipc-client.ts — Named Pipe 클라이언트 (boot-core ↔ Electron Main)

import net from 'net'

const CONNECT_TIMEOUT_MS = 2000
const REQUEST_TIMEOUT_MS = 3000

export class IpcClient {
  private pipePath: string
  private socket: net.Socket | null = null
  private connected = false
  private pendingResolve: ((v: unknown) => void) | null = null
  private pendingReject: ((e: Error) => void) | null = null
  private buffer = ''

  constructor(pipePath: string) {
    this.pipePath = pipePath
  }

  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const socket = net.createConnection(this.pipePath)
      socket.setTimeout(CONNECT_TIMEOUT_MS)

      socket.once('connect', () => {
        this.socket = socket
        this.connected = true
        socket.setTimeout(0)

        socket.on('data', (chunk) => this.onData(chunk))
        socket.on('error', () => this.handleDisconnect())
        socket.on('close', () => this.handleDisconnect())

        resolve()
      })

      socket.once('error', (e) => reject(e))
      socket.once('timeout', () => {
        socket.destroy()
        reject(new Error('pipe connect timeout'))
      })
    })
  }

  async request(cmd: string): Promise<unknown> {
    if (!this.connected || !this.socket) {
      throw new Error('not connected')
    }

    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pendingResolve = null
        this.pendingReject = null
        reject(new Error('request timeout'))
      }, REQUEST_TIMEOUT_MS)

      this.pendingResolve = (v) => {
        clearTimeout(timer)
        resolve(v)
      }
      this.pendingReject = (e) => {
        clearTimeout(timer)
        reject(e)
      }

      const payload = JSON.stringify({ cmd }) + '\n'
      this.socket!.write(payload)
    })
  }

  disconnect() {
    this.socket?.destroy()
    this.socket = null
    this.connected = false
  }

  private onData(chunk: Buffer) {
    this.buffer += chunk.toString('utf8')
    const lines = this.buffer.split('\n')
    this.buffer = lines.pop() ?? ''

    for (const line of lines) {
      if (!line.trim()) continue
      try {
        const parsed = JSON.parse(line)
        this.pendingResolve?.(parsed)
      } catch (e) {
        this.pendingReject?.(new Error(`invalid json: ${line}`))
      }
      this.pendingResolve = null
      this.pendingReject = null
    }
  }

  private handleDisconnect() {
    this.connected = false
    this.pendingReject?.(new Error('pipe disconnected'))
    this.pendingResolve = null
    this.pendingReject = null
  }
}
