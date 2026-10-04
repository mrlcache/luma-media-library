import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const child = spawn(process.execPath, [path.join(root,'node_modules/vite/bin/vite.js'), '--host', '127.0.0.1', '--port', '1422'], {
  cwd: root, windowsHide: true, stdio: 'inherit',
  env: {...process.env, VITE_LUMA_MOBILE:'true', VITE_LUMA_MOBILE_DEMO:'true'}
});
child.on('error', error => { console.error(error); process.exitCode = 1; });
child.on('exit', code => { process.exitCode = code ?? 1; });
for (const signal of ['SIGINT','SIGTERM']) process.on(signal, () => child.kill());
