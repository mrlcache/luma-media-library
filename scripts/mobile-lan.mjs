import { cp, mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import { networkInterfaces } from 'node:os';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const artifacts = path.join(root, '.artifacts/mobile-lan');
const statePath = path.join(artifacts, 'session.json');
const action = process.argv[2];
const requestedHost = process.argv[3];
const localAddresses = Object.values(networkInterfaces()).flat().filter(x => x && x.family === 'IPv4' && !x.internal).map(x => x.address);
let session;
try { session = JSON.parse(await readFile(statePath, 'utf8')); } catch (error) { if (error.code !== 'ENOENT') throw error; }
const host = requestedHost || session?.host;
if (!host || !localAddresses.includes(host)) {
  throw new Error(`Informe o IPv4 do PC na rede do celular. Enderecos disponiveis: ${localAddresses.join(', ')}. Exemplo: -PcAddress 192.168.1.16`);
}
if (session && action === 'web' && host !== session.host) {
  throw new Error(`O app DEV foi preparado para ${session.host}. Reinstale com o novo IP antes de iniciar o servidor.`);
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd: root, windowsHide: true, stdio: 'inherit', ...options });
    child.once('error', reject);
    child.once('exit', (code, signal) => code === 0 || signal === 'SIGINT' ? resolve() : reject(new Error(`Processo terminou: ${code ?? signal}`)));
    const stop = () => child.kill();
    process.once('SIGINT', stop);
    process.once('SIGTERM', stop);
    child.once('exit', () => { process.off('SIGINT', stop); process.off('SIGTERM', stop); });
  });
}

if (action === 'web') {
  console.log(`Interface Wi-Fi DEV: http://${host}:1424 (PC e celular na mesma rede). Ctrl+C encerra apenas este servidor.`);
  await run(process.execPath, [path.join(root, 'node_modules/vite/bin/vite.js'), '--config', 'scripts/vite.mobile-lan.config.ts'], {
    env: { ...process.env, LUMA_LAN_HOST: host, VITE_LUMA_MOBILE: 'true', VITE_LUMA_MOBILE_DEMO: 'false' }
  });
} else if (action === 'prepare') {
  // Unique snapshot: no shared Android generated files, Cargo target or release outputs.
  const snapshot = path.join(artifacts, `native-${Date.now()}`, 'src-tauri');
  await mkdir(snapshot, { recursive: true });
  const source = path.join(root, 'mobile/src-tauri');
  for (const name of ['Cargo.toml', 'Cargo.lock', 'build.rs', 'src', 'capabilities', 'gen/android', 'tauri.conf.json']) {
    await cp(path.join(source, name), path.join(snapshot, name), { recursive: true,
      filter: file => !file.split(path.sep).some(part => ['target', 'build', '.gradle', '.cxx', 'jniLibs'].includes(part)) });
  }
  // Package substitutions apply only to the snapshot, including the native plugin class.
  async function patchTree(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const file = path.join(directory, entry.name);
      if (entry.isDirectory()) await patchTree(file);
      else if (/\.(rs|kt|kts|json|xml|gradle|properties)$/.test(entry.name)) {
        const original = await readFile(file, 'utf8');
        let updated = original.replaceAll('app.luma.mobile.demo', 'app.luma.mobile.wifidev');
        if (entry.name === 'strings.xml') updated = updated.replace(/(<string name="(?:app_name|main_activity_title)">)[^<]*/g, '$1Luma Wi-Fi DEV');
        if (entry.name === 'BuildTask.kt') updated = updated.replace('"../../node_modules/@tauri-apps/cli/tauri.js"', JSON.stringify(path.join(root, 'node_modules/@tauri-apps/cli/tauri.js').replaceAll('\\', '/')));
        if (original !== updated) await writeFile(file, updated);
      }
    }
  }
  await patchTree(snapshot);
  const base = JSON.parse(await readFile(path.join(snapshot, 'tauri.conf.json'), 'utf8'));
  const overlay = JSON.parse(await readFile(path.join(source, 'tauri.mobile-lan.json'), 'utf8'));
  const config = { ...base, ...overlay, build: { ...base.build, ...overlay.build,
    devUrl: `http://${host}:1424`, frontendDist: path.join(root, '.artifacts/mobile-lan/web') },
    bundle: { ...base.bundle, icon: [path.join(root, 'src-tauri/icons/icon.png')] } };
  await writeFile(path.join(snapshot, 'tauri.conf.json'), JSON.stringify(config, null, 2));
  session = { host, port: 1424, snapshot, identifier: overlay.identifier, preparedAt: new Date().toISOString() };
  await writeFile(statePath, JSON.stringify(session, null, 2));
  console.log(`Copia DEV preparada: ${snapshot}\nIP gravado no app: ${host}:1424. Nenhuma compilacao foi iniciada.`);
} else {
  throw new Error('Use web ou prepare. Para instalar, use start-android-lan-dev.ps1 -Install.');
}
