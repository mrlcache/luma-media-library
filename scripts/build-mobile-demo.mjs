import { spawn } from 'node:child_process';
import { cp } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const demo = process.env.LUMA_MOBILE_REAL !== 'true';
const npm = process.env.npm_execpath || path.join(path.dirname(process.execPath), 'node_modules/npm/bin/npm-cli.js');
await new Promise((resolve, reject) => {
  const child = spawn(process.execPath, [npm, 'run', 'build'], {cwd:root, windowsHide:true, stdio:'inherit', env:{...process.env,VITE_LUMA_MOBILE:'true',VITE_LUMA_MOBILE_DEMO:demo?'true':'false'}});
  child.on('error', reject); child.on('exit', code => code === 0 ? resolve() : reject(new Error(`Mobile build failed: ${code}`)));
});
if (demo) await cp(path.join(root,'mobile/demo-assets'),path.join(root,'.artifacts/mobile-web/mobile-demo'),{recursive:true});
await new Promise((resolve, reject) => {
  const child = spawn(process.execPath, [path.join(root,'scripts/test-mobile-runtime.mjs'),...(demo?[]:['--real'])], {cwd:root, windowsHide:true, stdio:'inherit'});
  child.on('error', reject); child.on('exit', code => code === 0 ? resolve() : reject(new Error('Mobile UI startup test failed; Android compilation was skipped.')));
});
