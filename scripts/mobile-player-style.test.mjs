import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { build } from 'vite';

test('mobile blur reset survives Chrome 87 production CSS optimization', async () => {
  const source = await readFile(new URL('../src/lib/components/PlayerOverlay.svelte', import.meta.url), 'utf8');
  const rule = source.match(/\.player-overlay--mobile :global\(\*\)\s*\{[^}]+\}/)?.[0];
  assert.ok(rule, 'Mobile player needs its backdrop reset');
  const directory = await mkdtemp(path.join(tmpdir(), 'luma-player-css-'));
  assert.equal(path.dirname(path.resolve(directory)), path.resolve(tmpdir()));
  try {
    await writeFile(path.join(directory, 'style.css'), rule.replace(':global(*)', '*'));
    await writeFile(path.join(directory, 'entry.js'), "import './style.css';");
    const result = await build({
      configFile: false, root: directory, logLevel: 'silent',
      build: { write: false, target: 'chrome87', rollupOptions: { input: path.join(directory, 'entry.js') } }
    });
    const css = result.output.filter(item => item.type === 'asset' && item.fileName.endsWith('.css')).map(item => String(item.source)).join('\n');
    assert.match(css, /(?<!-webkit-)backdrop-filter\s*:\s*none\s*!important/, 'The Android reset must keep the unprefixed declaration');
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
