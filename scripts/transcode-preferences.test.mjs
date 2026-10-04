import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import ts from 'typescript';

const source = readFileSync(new URL('../src/lib/platform/transcode-preferences.ts', import.meta.url), 'utf8');
const code = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
function fixture() {
  const values = new Map();
  const exports = {};
  new Function('exports', 'localStorage', code)(exports, { getItem: key => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) });
  return { ...exports, values };
}
test('retains mode, quality and bitrate between player openings, including switching off', () => {
  const f = fixture();
  const selected = { enabled: true, quality: '720p', bitrate: 4000000 };
  f.saveTranscodePreferences(selected);
  assert.deepEqual(f.readTranscodePreferences(), selected);
  f.saveTranscodePreferences({ ...selected, enabled: false });
  assert.deepEqual(f.readTranscodePreferences(), { ...selected, enabled: false });
});
test('invalid or corrupt saved options fall back to supported presets', () => {
  const f = fixture();
  f.values.set('luma.mobile.transcoding', '{');
  assert.deepEqual(f.readTranscodePreferences(), { enabled: false, quality: 'auto', bitrate: 0 });
  f.saveTranscodePreferences({ enabled: true, quality: '1080p', bitrate: 999 });
  assert.equal(f.readTranscodePreferences().bitrate, 5000000);
  f.saveTranscodePreferences({ enabled: true, quality: 'unknown', bitrate: 999 });
  assert.equal(f.readTranscodePreferences().quality, 'auto');
});
