import { createConnection } from 'node:net';
import { homedir } from 'node:os';
import { join } from 'node:path';

export function socketPath() {
  if (process.env.STARPERSONA_SOCKET) return process.env.STARPERSONA_SOCKET;
  if (process.platform === 'win32') throw new Error('本地 AI 接口当前支持 macOS / Linux，Windows 管道尚未实现');
  const base = process.platform === 'darwin'
    ? join(homedir(), 'Library', 'Application Support')
    : process.env.XDG_DATA_HOME || join(homedir(), '.local', 'share');
  return join(base, 'app.starpersona.desktop', 'control.sock');
}

export function request(method, params = {}, path = socketPath()) {
  return new Promise((resolve, reject) => {
    const socket = createConnection(path);
    socket.setEncoding('utf8');
    let bytes = '';
    socket.setTimeout(15000);
    socket.on('connect', () => socket.write(`${JSON.stringify({ method, params })}\n`));
    socket.on('timeout', () => socket.destroy(new Error('星相响应超时；请检查桌面窗口和浏览器状态')));
    socket.on('error', error => reject(new Error(
      ['ENOENT', 'ECONNREFUSED'].includes(error.code) ? '星相尚未启动，请先打开新版 StarPersona.app' : error.message,
    )));
    socket.on('data', chunk => {
      bytes += chunk;
      if (bytes.length > 32 * 1024 * 1024) { socket.destroy(new Error('控制响应过大')); return; }
      if (!bytes.includes('\n')) return;
      socket.end();
      try {
        const response = JSON.parse(bytes.slice(0, bytes.indexOf('\n')));
        if (response.error) reject(new Error(response.error));
        else resolve(response.result);
      } catch (error) { reject(error); }
    });
    socket.on('end', () => { if (!bytes.includes('\n')) reject(new Error('星相返回了不完整的响应')); });
  });
}
