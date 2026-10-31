// Minimal obs-websocket v5 client (OBS 28+), using the WebSocket built into Node 22+.

import { createHash, randomUUID } from 'node:crypto';

const sha256b64 = (text) => createHash('sha256').update(text).digest('base64');

export class Obs {
  #ws = null;
  #pending = new Map();

  /** Connects and identifies; rejects with a readable message. */
  async connect(url, password) {
    const ws = new WebSocket(url, 'obswebsocket.json');
    this.#ws = ws;
    await new Promise((resolve, reject) => {
      const fail = (message) => {
        reject(new Error(message));
        ws.close();
      };
      ws.onerror = () => fail(`no OBS WebSocket server at ${url}`);
      ws.onclose = (event) => {
        if (event.code === 4009) fail('OBS rejected the password (set OBS_PASSWORD)');
        else fail(`OBS closed the connection (${event.code})`);
      };
      ws.onmessage = (event) => {
        const { op, d } = JSON.parse(event.data);
        if (op === 0) {
          const identify = { rpcVersion: 1, eventSubscriptions: 0 };
          if (d.authentication) {
            if (!password) return fail('OBS needs a password (set OBS_PASSWORD)');
            const secret = sha256b64(password + d.authentication.salt);
            identify.authentication = sha256b64(secret + d.authentication.challenge);
          }
          ws.send(JSON.stringify({ op: 1, d: identify }));
        } else if (op === 2) {
          resolve();
        }
      };
    });
    ws.onmessage = (event) => {
      const { op, d } = JSON.parse(event.data);
      if (op !== 7) return;
      const pending = this.#pending.get(d.requestId);
      if (!pending) return;
      this.#pending.delete(d.requestId);
      if (d.requestStatus.result) pending.resolve(d.responseData ?? {});
      else pending.reject(new Error(`${d.requestType}: ${d.requestStatus.comment ?? d.requestStatus.code}`));
    };
    ws.onclose = () => {
      this.#ws = null;
      for (const { reject } of this.#pending.values()) reject(new Error('OBS disconnected'));
      this.#pending.clear();
      this.onclose?.();
    };
  }

  get connected() {
    return this.#ws?.readyState === WebSocket.OPEN;
  }

  call(requestType, requestData = {}) {
    if (!this.connected) return Promise.reject(new Error('OBS is not connected'));
    const requestId = randomUUID();
    return new Promise((resolve, reject) => {
      this.#pending.set(requestId, { resolve, reject });
      this.#ws.send(JSON.stringify({ op: 6, d: { requestType, requestId, requestData } }));
    });
  }

  close() {
    this.#ws?.close();
  }
}
