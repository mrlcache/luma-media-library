import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { copyFile, stat } from 'node:fs/promises';

async function digest(file) {
    const hash = createHash('sha256');
    for await (const chunk of createReadStream(file)) hash.update(chunk);
    return hash.digest('hex');
}

// Loaded Windows DLLs/executables cannot be overwritten. Keep identical runtimes
// untouched so preparing a release does not require closing the development app.
export async function copyRuntimeFile(source, destination) {
    const existing = await stat(destination).catch(error => {
        if (error.code === 'ENOENT') return null;
        throw error;
    });
    if (existing && existing.size === (await stat(source)).size && await digest(source) === await digest(destination)) return;
    await copyFile(source, destination);
}
