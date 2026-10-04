import { DatabaseSync } from 'node:sqlite';
import { realpath, stat } from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';

const VIDEO = new Set(['avi','flv','m2ts','m4v','mkv','mov','mp4','mpeg','mpg','mts','ts','webm','wmv']);
export const mime = ext => ({mkv:'video/x-matroska',webm:'video/webm',avi:'video/avi',mov:'video/quicktime',ts:'video/mp2t',m2ts:'video/mp2t',mts:'video/mp2t',mpeg:'video/mpeg',mpg:'video/mpeg',wmv:'video/x-ms-wmv',flv:'video/x-flv'}[ext] || 'video/mp4');

export async function safeFile(root, relative) {
  const canonicalRoot = await realpath(root);
  const canonicalFile = await realpath(path.resolve(root, relative));
  const remainder = path.relative(canonicalRoot, canonicalFile);
  if (!remainder || remainder === '..' || remainder.startsWith(`..${path.sep}`) || path.isAbsolute(remainder)) throw new Error('File outside library root');
  const info = await stat(canonicalFile);
  if (!info.isFile()) throw new Error('Not a file');
  return { path: canonicalFile, info };
}

export class Catalog {
  constructor(dbPath) { this.dbPath = dbPath; this.items = []; this.updateId = 0; this.fingerprint = ''; this.lastRefresh = 0; }
  async refresh(force = false) {
    if (!force && Date.now() - this.lastRefresh < 5000) return;
    const db = new DatabaseSync(this.dbPath, {readOnly:true, timeout:2000});
    let rows;
    try {
      rows = db.prepare(`SELECT i.id, r.canonical_path AS root, i.relative_path AS relative,
        f.display_name AS fileName, lower(f.extension) AS extension, f.size_bytes AS size,
        COALESCE(NULLIF(m.title, ''), NULLIF(i.local_title, ''), f.display_name) AS title,
        COALESCE(m.kind, i.local_kind, 'movie') AS kind,
        COALESCE(m.release_year, i.local_year) AS year, m.tmdb_id AS tmdbId,
        m.poster_url AS poster, m.overview AS overview, i.local_key AS localKey
        FROM media_items i JOIN library_roots r ON r.id=i.root_id
        JOIN media_files f ON f.root_id=i.root_id AND f.relative_path=i.relative_path AND f.generation=r.current_generation
        LEFT JOIN media_metadata m ON m.media_id=i.id ORDER BY title COLLATE NOCASE, relative`).all();
    } finally { db.close(); }
    const items = rows.filter(row => VIDEO.has(row.extension)).map(row => {
      const position = `${row.relative} ${row.fileName}`.match(/(?:s(\d{1,2})[ ._-]*e(\d{1,3})|(\d{1,2})x(\d{1,3}))/i);
      const season = position ? Number(position[1] || position[3]) : null;
      const episode = position ? Number(position[2] || position[4]) : null;
      const seriesKey = row.tmdbId ? `tmdb-${row.tmdbId}` : `local-${createHash('sha256').update(`${row.localKey || row.title}:${row.year || ''}`).digest('hex').slice(0,16)}`;
      return {...row, id:`file-${row.id}`, mediaId:row.id, mime:mime(row.extension), season, episode,
        parentID:row.kind === 'series' ? `series-${seriesKey}` : 'movies',
        displayTitle:row.kind === 'series' ? `${row.title}${position ? ` · S${String(season).padStart(2,'0')}E${String(episode).padStart(2,'0')}` : ` · ${row.fileName}`}` : row.title};
    });
    const fingerprint = createHash('sha256').update(JSON.stringify(items)).digest('hex');
    if (fingerprint !== this.fingerprint) { this.updateId = (this.updateId + 1) >>> 0; this.fingerprint = fingerprint; }
    this.items = items;
    this.lastRefresh = Date.now();
  }
  containers() {
    const shows = new Map();
    for (const item of this.items.filter(i => i.kind === 'series')) {
      if (!shows.has(item.parentID)) shows.set(item.parentID, {id:item.parentID,parentID:'series',title:item.title,poster:item.poster,container:true});
    }
    const basics = [{id:'0',parentID:'-1',title:'Luma',container:true},{id:'movies',parentID:'0',title:'Movies',container:true},{id:'series',parentID:'0',title:'Series',container:true}];
    return [...basics,...shows.values()].map(c => ({...c,childCount:c.id === '0' ? 2 : c.id === 'series' ? shows.size : this.items.filter(i => i.parentID === c.id).length}));
  }
  browse(id, metadata = false) {
    const containers = this.containers();
    const object = containers.find(c => c.id === id) || this.items.find(i => i.id === id);
    if (!object) return null;
    if (metadata) return [object];
    if (!object.container) return null;
    return [...containers.filter(c => c.parentID === id),...this.items.filter(i => i.parentID === id)];
  }
  async file(id) {
    await this.refresh();
    const item = this.items.find(i => i.id === id);
    if (!item) return null;
    try { return {...item,...await safeFile(item.root,item.relative)}; } catch { return null; }
  }
  publicItems() { return this.items.map(({id,displayTitle,kind,year,poster,size,mime}) => ({id,title:displayTitle,kind,year,poster,size,mime})); }
}
