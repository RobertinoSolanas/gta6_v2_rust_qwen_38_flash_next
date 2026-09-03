// Minimal Chrome DevTools Protocol client — no third-party dependencies.
//
// Launches headless Chrome with --remote-debugging-port, discovers the ws URL
// from the stderr banner, then speaks RFC 6455 directly over the raw socket.
// Node ships no built-in WebSocket client for Node >= 22 reliable use here, so
// the (small) framing code lives in this file to keep the repo dependency-free.

import http from "node:http";
import crypto from "node:crypto";
import fs from "node:fs";
import { spawn } from "node:child_process";

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const CHROME_FLAGS = [
  "--headless=new",
  "--remote-debugging-port=0",
  "--remote-allow-origins=*",
  "--no-first-run",
  "--no-default-browser-check",
  "--disable-extensions",
  "--no-sandbox",
  "--mute-audio",
  "--hide-scrollbars",
  "--disable-background-timer-throttling",
  "--disable-renderer-backgrounding",
  "--disable-backgrounding-occluded-windows",
  "--window-size=1280,720",
  // software WebGL so WebGL2 exists on machines without a GPU
  "--use-gl=angle",
  "--use-angle=swiftshader",
  "--enable-unsafe-swiftshader",
  "--ignore-gpu-blocklist",
];

function chromeBin() {
  return process.env.CHROME_BIN || process.env.GOOGLE_CHROME_BIN || "google-chrome";
}

async function fetchJson(url, retries = 100) {
  let lastErr;
  for (let i = 0; i < retries; i++) {
    try {
      const res = await fetch(url);
      if (res.ok) return await res.json();
      lastErr = new Error(`${url} -> HTTP ${res.status}`);
    } catch (err) {
      lastErr = err;
    }
    await sleep(150);
  }
  throw lastErr;
}

// ---------------------------------------------------------------------------
// minimal websocket
// ---------------------------------------------------------------------------

class MiniWebSocket {
  constructor(socket) {
    this.socket = socket;
    this.buffer = Buffer.alloc(0);
    this.onmessage = null;
    this.onclose = null;
    socket.setNoDelay(true);
    socket.on("data", (chunk) => {
      this.buffer = Buffer.concat([this.buffer, chunk]);
      this.drain();
    });
    const closed = () => this.onclose && this.onclose();
    socket.on("close", closed);
    socket.on("error", closed);
  }

  drain() {
    for (;;) {
      if (this.buffer.length < 2) return;
      const opcode = this.buffer[0] & 0x0f;
      const masked = (this.buffer[1] & 0x80) !== 0;
      let len = this.buffer[1] & 0x7f;
      let offset = 2;
      if (len === 126) {
        if (this.buffer.length < offset + 2) return;
        len = this.buffer.readUInt16BE(offset);
        offset += 2;
      } else if (len === 127) {
        if (this.buffer.length < offset + 8) return;
        len = Number(this.buffer.readBigUInt64BE(offset));
        offset += 8;
      }
      let maskKey = null;
      if (masked) {
        if (this.buffer.length < offset + 4) return;
        maskKey = Buffer.from(this.buffer.subarray(offset, offset + 4));
        offset += 4;
      }
      if (this.buffer.length < offset + len) return;
      let payload = Buffer.from(this.buffer.subarray(offset, offset + len));
      this.buffer = this.buffer.subarray(offset + len);
      if (maskKey) {
        for (let i = 0; i < payload.length; i++) payload[i] ^= maskKey[i % 4];
      }
      if (opcode === 0x8) {
        this.onclose && this.onclose();
        return;
      }
      if (opcode === 0x9) {
        this.sendFrame(0xa, payload); // pong
        continue;
      }
      if (opcode === 0x1 || opcode === 0x2) {
        this.onmessage && this.onmessage(payload.toString("utf8"));
      }
    }
  }

  sendFrame(opcode, payload) {
    const data = Buffer.from(payload);
    const mask = crypto.randomBytes(4);
    let header;
    if (data.length < 126) {
      header = Buffer.from([0x80 | opcode, 0x80 | data.length]);
    } else if (data.length < 65536) {
      header = Buffer.alloc(4);
      header[0] = 0x80 | opcode;
      header[1] = 0x80 | 126;
      header.writeUInt16BE(data.length, 2);
    } else {
      header = Buffer.alloc(10);
      header[0] = 0x80 | opcode;
      header[1] = 0x80 | 127;
      header.writeBigUInt64BE(BigInt(data.length), 2);
    }
    const maskedData = Buffer.from(data);
    for (let i = 0; i < maskedData.length; i++) maskedData[i] ^= mask[i % 4];
    this.socket.write(Buffer.concat([header, mask, maskedData]));
  }

  send(text) {
    this.sendFrame(0x1, Buffer.from(text, "utf8"));
  }

  close() {
    try {
      this.sendFrame(0x8, Buffer.alloc(0));
    } catch {
      /* ignore */
    }
    try {
      this.socket.destroy();
    } catch {
      /* ignore */
    }
  }
}

function connectWebSocket(url) {
  const u = new URL(url);
  const key = crypto.randomBytes(16).toString("base64");
  return new Promise((resolve, reject) => {
    const req = http.request({
      host: u.hostname,
      port: u.port || 80,
      path: u.pathname + u.search,
      method: "GET",
      headers: {
        Connection: "Upgrade",
        Upgrade: "websocket",
        "Sec-WebSocket-Key": key,
        "Sec-WebSocket-Version": "13",
      },
    });
    req.on("upgrade", (_res, socket) => resolve(new MiniWebSocket(socket)));
    req.on("response", (res) =>
      reject(new Error(`websocket upgrade failed: HTTP ${res.statusCode}`)),
    );
    req.on("error", reject);
    req.end();
  });
}

// ---------------------------------------------------------------------------
// session
// ---------------------------------------------------------------------------

export class Chrome {
  constructor(proc, ws, profileDir) {
    this.proc = proc;
    this.ws = ws;
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Map();
    this.consoleMessages = [];
    this.pageErrors = [];
    this.ws.onmessage = (text) => this.dispatch(text);
    this.on("Runtime.consoleAPICalled", (p) => {
      this.consoleMessages.push({
        type: p.type,
        text: (p.args || []).map((a) => a.description ?? a.value).join(" "),
      });
    });
    this.on("Runtime.exceptionThrown", (p) => {
      const d = p.exceptionDetails || {};
      this.pageErrors.push(
        (d.exception && (d.exception.description || d.exception.value)) || d.text || "exception",
      );
    });
    this.on("Log.entryAdded", (p) => {
      const e = p.entry || {};
      if (e.level !== "error") return;
      // browser-level noise that the page itself cannot influence
      if (/404|Failed to load resource/i.test(e.text || "")) return;
      this.consoleMessages.push({ type: e.source || "log", text: e.text });
    });
  }

  static async launch(extraFlags = []) {
    const profileDir = fs.mkdtempSync("/tmp/neonbay-chrome-");
    const proc = spawn(
      chromeBin(),
      [...CHROME_FLAGS, ...extraFlags, `--user-data-dir=${profileDir}`, "about:blank"],
      { stdio: ["ignore", "ignore", "pipe"] },
    );
    let stderr = "";
    proc.stderr.on("data", (d) => {
      stderr += d.toString();
    });
    proc.on("exit", (code) => {
      if (!/DevTools listening on/.test(stderr)) {
        // eslint-disable-next-line no-console
        console.error(`chrome exited early (${code})\n${stderr}`);
      }
    });
    const deadline = Date.now() + 45000;
    let wsUrl = null;
    while (Date.now() < deadline && !wsUrl) {
      const m = stderr.match(/DevTools listening on (ws:\/\/\S+)/);
      if (m) wsUrl = m[1];
      else await sleep(120);
    }
    if (!wsUrl) throw new Error(`Chrome did not expose DevTools.\nstderr:\n${stderr}`);
    return { proc, wsUrl, profileDir };
  }

  static async start(extraFlags = []) {
    const { proc, wsUrl, profileDir } = await Chrome.launch(extraFlags);
    // The browser-level ws gives us the target list; use the http endpoint instead.
    const port = new URL(wsUrl).port;
    const targets = await fetchJson(`http://127.0.0.1:${port}/json/list`);
    const page = targets.find((t) => t.type === "page") || targets[0];
    const ws = await connectWebSocket(page.webSocketDebuggerUrl);
    const chrome = new Chrome(proc, ws, profileDir);
    chrome.browserWsUrl = wsUrl;
    return chrome;
  }

  dispatch(text) {
    let msg;
    try {
      msg = JSON.parse(text);
    } catch {
      return;
    }
    if (msg.id !== undefined && this.pending.has(msg.id)) {
      const { resolve, reject } = this.pending.get(msg.id);
      this.pending.delete(msg.id);
      if (msg.error) reject(new Error(`${msg.method}: ${JSON.stringify(msg.error)}`));
      else resolve(msg.result);
      return;
    }
    if (msg.method) {
      const params = msg.params || {};
      for (const fn of this.listeners.get(msg.method) || []) fn(params);
      for (const fn of this.listeners.get("*") || []) fn(msg);
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    const payload = JSON.stringify({ id, method, params });
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      try {
        this.ws.send(payload);
      } catch (err) {
        this.pending.delete(id);
        reject(err);
      }
    });
  }

  on(method, fn) {
    if (!this.listeners.has(method)) this.listeners.set(method, new Set());
    this.listeners.get(method).add(fn);
    return () => this.listeners.get(method).delete(fn);
  }

  /** Evaluate in the page; returns the JSON-serialised value. */
  async evaluate(expression, { awaitPromise = true } = {}) {
    const res = await this.send("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise,
    });
    if (res.exceptionDetails) {
      const d = res.exceptionDetails;
      const detail =
        (res.result && (res.result.description || res.result.value)) ||
        (res.exceptionDetails.exception &&
          (res.exceptionDetails.exception.description ||
            res.exceptionDetails.exception.value)) ||
        res.exceptionDetails.text;
      throw new Error(`page eval error: ${detail}`);
    }
    return res.result ? res.result.value : undefined;
  }

  async goto(url, { timeoutMs = 90000 } = {}) {
    const loaded = new Promise((resolve, reject) => {
      const offLoad = this.on("Page.loadEventFired", () => {
        offLoad();
        resolve(true);
      });
    });
    await this.send("Page.navigate", { url });
    const winner = await Promise.race([loaded, sleep(timeoutMs).then(() => "timeout")]);
    if (winner !== true) throw new Error(`timeout loading ${url}`);
  }

  /** Enable the domains we listen on, then navigate. */
  async open(url, { timeoutMs = 90000 } = {}) {
    await this.send("Runtime.enable");
    await this.send("Log.enable").catch(() => {});
    await this.send("Page.enable");
    await this.goto(url, { timeoutMs });
  }

  async screenshot(path) {
    const res = await this.send("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync(path, Buffer.from(res.data, "base64"));
    return path;
  }

  async stop() {
    try {
      this.ws.close();
    } catch {
      /* ignore */
    }
    try {
      this.proc.kill("SIGKILL");
    } catch {
      /* ignore */
    }
    try {
      fs.rmSync(this.profileDir, { recursive: true, force: true });
    } catch {
      /* ignore */
    }
  }
}

/** Poll a JS predicate until it returns truthy, or throw after `timeoutMs`. */
export async function waitFor(
  check,
  { timeoutMs = 30000, intervalMs = 150, what = "condition" } = {},
) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await check();
    if (value) return value;
    if (Date.now() > deadline) {
      throw new Error(`timeout waiting for ${what} (${timeoutMs}ms)`);
    }
    await sleep(intervalMs);
  }
}

/** Poll a page expression until it evaluates truthy. */
export async function waitForInPage(chrome, expression, opts = {}) {
  return waitFor(() => chrome.evaluate(`!!(${expression})`, { awaitPromise: false }), {
    what: `page: ${expression}`,
    ...opts,
  });
}
