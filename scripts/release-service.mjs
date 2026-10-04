// Desktop-owned loopback service; no Vite or public HTTP listener is required.
import {createServer} from 'node:http';
import {createReleaseSearch} from './torrent-search-preview.mjs';
const search=createReleaseSearch();
createServer(async(req,res)=>{
  res.setHeader('Content-Type','application/json');
  const url=new URL(req.url || '/', 'http://127.0.0.1:8943');
  if(req.method !== 'GET' || req.headers['x-luma-control'] !== '1' || url.pathname !== '/api/releases') {
    res.writeHead(404); res.end('{"error":"Not found"}'); return;
  }
  try{res.end(JSON.stringify(await search(url.searchParams)));}
  catch(error){res.writeHead(502);res.end(JSON.stringify({error:String(error.message || error)}));}
}).listen(8943,'127.0.0.1');
