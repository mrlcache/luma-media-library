import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import { createWriteStream, existsSync } from 'node:fs';
import { mkdir, readdir, stat, copyFile, rename, rm, readFile } from 'node:fs/promises';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { promisify } from 'node:util';
import { dirname, extname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import sevenZip from '7zip-bin';

const run = promisify(execFile);
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const targetRoot = resolve(repoRoot, 'src-tauri', 'target');
const version = '20260928';
const filename = 'mpv-dev-x86_64-20260928-git-e470f8986e.7z';
const expectedSha256 = '81795d759e01016f1550fd71651a1a5d59ab5c28ef31c0b6793224e9cff39459';
const archivePath = resolve(targetRoot, 'mpv-downloads', filename);
const extractionPath = resolve(targetRoot, 'mpv-extracted', version);
const enginePath = resolve(targetRoot, 'player-engines');
const debugEnginePath = resolve(targetRoot, 'debug', 'player-engines');

for (const path of [archivePath, extractionPath, enginePath, debugEnginePath]) {
	if (!path.startsWith(`${targetRoot}${sep}`)) throw new Error(`Refusing to write outside the Tauri target directory: ${path}`);
}

await mkdir(dirname(archivePath), { recursive: true });
if (!existsSync(archivePath) || (await stat(archivePath)).size === 0) {
	const temporaryArchivePath = `${archivePath}.download`;
	try {
		const response = await fetch(`https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/${version}/${filename}`);
		if (!response.ok || !response.body) throw new Error(`MPV download failed with HTTP ${response.status}.`);
		await pipeline(Readable.fromWeb(response.body), createWriteStream(temporaryArchivePath));
		await rename(temporaryArchivePath, archivePath);
	} catch (error) {
		await rm(temporaryArchivePath, { force: true });
		throw error;
	}
}

const archiveHash = createHash('sha256').update(await readFile(archivePath)).digest('hex');
if (archiveHash !== expectedSha256) throw new Error(`MPV archive checksum mismatch: expected ${expectedSha256}, got ${archiveHash}.`);

await mkdir(extractionPath, { recursive: true });
await run(sevenZip.path7za, ['x', archivePath, `-o${extractionPath}`, '-y', '-bso0', '-bsp0']);

async function findMpvLibrary(directory) {
	for (const entry of await readdir(directory, { withFileTypes: true })) {
		const path = join(directory, entry.name);
		if (entry.isFile() && entry.name.toLowerCase() === 'libmpv-2.dll') return path;
		if (entry.isDirectory()) {
			const found = await findMpvLibrary(path);
			if (found) return found;
		}
	}
	return null;
}

const mpvLibrary = await findMpvLibrary(extractionPath);
if (!mpvLibrary) throw new Error('The pinned MPV development package did not contain libmpv-2.dll.');
const runtimeDirectory = dirname(mpvLibrary);
const runtimeFiles = await readdir(runtimeDirectory, { withFileTypes: true });
const runtimeDlls = runtimeFiles.filter((entry) => entry.isFile() && extname(entry.name).toLowerCase() === '.dll');
if (!runtimeDlls.some((entry) => entry.name.toLowerCase() === 'libmpv-2.dll')) throw new Error('libmpv-2.dll is missing from its runtime folder.');

await mkdir(enginePath, { recursive: true });
await mkdir(debugEnginePath, { recursive: true });
for (const runtimeDll of runtimeDlls) {
	await copyFile(join(runtimeDirectory, runtimeDll.name), join(enginePath, runtimeDll.name));
	await copyFile(join(runtimeDirectory, runtimeDll.name), join(debugEnginePath, runtimeDll.name));
}
console.log(`Prepared MPV ${version}: libmpv-2.dll and ${runtimeDlls.length - 1} runtime DLL(s).`);
