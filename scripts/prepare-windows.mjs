import { existsSync } from 'node:fs';
import { readdir, stat, mkdir, copyFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const bridgeRoot = join(repoRoot, 'native', 'libtorrent-bridge');
const bridgeBuild = join(bridgeRoot, 'build');
const bridgeDll = join(bridgeBuild, 'Release', 'media_libtorrent_bridge.dll');

// Bundle the runtime and resolver so release search also works after installation.
const releaseRuntime = join(repoRoot, 'src-tauri', 'target', 'release-service');
await mkdir(releaseRuntime, {recursive:true});
await copyFile(process.execPath, join(releaseRuntime,'node.exe'));
for(const file of ['release-service.mjs','torrent-search-preview.mjs','prowlarr-preview.mjs']) {
    await copyFile(join(repoRoot,'scripts',file),join(releaseRuntime,file));
}

function run(command, args) {
	return new Promise((resolvePromise, rejectPromise) => {
		const child = spawn(command, args, { cwd: repoRoot, stdio: 'inherit', windowsHide: true });
		child.once('error', rejectPromise);
		child.once('close', (code) => code === 0
			? resolvePromise()
			: rejectPromise(new Error(`${command} exited with code ${code}.`)));
	});
}

await run(process.execPath, [join(repoRoot, 'scripts', 'prepare-mpv.mjs')]);

async function newestSourceTime(path) {
	let newest = (await stat(path)).mtimeMs;
	for (const entry of await readdir(path, { withFileTypes: true })) {
		if (entry.name === 'build') continue;
		const child = join(path, entry.name);
		const childStat = await stat(child);
		newest = Math.max(newest, childStat.mtimeMs);
		if (entry.isDirectory()) newest = Math.max(newest, await newestSourceTime(child));
	}
	return newest;
}

if (existsSync(bridgeDll) && (await stat(bridgeDll)).mtimeMs >= await newestSourceTime(bridgeRoot)) {
	console.log('The native libtorrent bridge is already up to date.');
	process.exit(0);
}

const vcpkgRoot = process.env.VCPKG_ROOT || process.env.VCPKG_INSTALLATION_ROOT;
if (!vcpkgRoot || !existsSync(join(vcpkgRoot, 'scripts', 'buildsystems', 'vcpkg.cmake'))) {
	if (existsSync(bridgeDll)) {
		console.warn('The checked-in native bridge sources changed, but vcpkg was not found; keeping the existing local bridge.');
		process.exit(0);
	}
	throw new Error('Building Luma requires Visual Studio C++ tools, CMake, and vcpkg. Set VCPKG_ROOT to the vcpkg installation, then run this build again.');
}

await run('cmake', [
	'-S', bridgeRoot,
	'-B', bridgeBuild,
	`-DCMAKE_TOOLCHAIN_FILE=${join(vcpkgRoot, 'scripts', 'buildsystems', 'vcpkg.cmake')}`,
	'-DVCPKG_TARGET_TRIPLET=x64-windows-static-md'
]);
await run('cmake', ['--build', bridgeBuild, '--config', 'Release']);

if (!existsSync(bridgeDll) || !bridgeDll.startsWith(`${resolve(bridgeRoot, 'build')}${sep}`)) {
	throw new Error('The native libtorrent bridge build did not produce its expected DLL.');
}
console.log('Built the native libtorrent bridge.');
