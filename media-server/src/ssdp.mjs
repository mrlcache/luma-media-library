import dgram from 'node:dgram';
import { CD, CM, DEVICE } from './protocol.mjs';

const GROUP = '239.255.255.250';
export const targets = uuid => ['upnp:rootdevice',`uuid:${uuid}`,DEVICE,CD,CM];
export function discoveryMessage({uuid,location,target,alive = true,response = false}) {
  const usn = target === `uuid:${uuid}` ? target : `uuid:${uuid}::${target}`;
  const fields = response ? ['HTTP/1.1 200 OK','EXT:',`ST: ${target}`] : ['NOTIFY * HTTP/1.1',`HOST: ${GROUP}:1900`,`NT: ${target}`,`NTS: ssdp:${alive ? 'alive' : 'byebye'}`];
  if (alive || response) fields.push('CACHE-CONTROL: max-age=180',`LOCATION: ${location}`,'SERVER: Windows/10 UPnP/1.0 Luma/0.1.0');
  fields.push(`USN: ${usn}`);
  if (response) fields.push(`DATE: ${new Date().toUTCString()}`);
  return `${fields.join('\r\n')}\r\n\r\n`;
}

export class Discovery {
  constructor(options) { this.options = options; this.pending = new Set(); this.socket = null; this.lastError = null; }
  async start() {
    const socket = dgram.createSocket({type:'udp4',reuseAddr:true});
    this.socket = socket;
    socket.on('error', error => { this.lastError = error.message; });
    await new Promise((resolve,reject) => { socket.once('error',reject); socket.bind(1900,'0.0.0.0',() => {socket.removeListener('error',reject);resolve();}); });
    try { socket.addMembership(GROUP,this.options.host); socket.setMulticastInterface(this.options.host); socket.setMulticastTTL(2); }
    catch (error) { socket.close(); this.socket = null; throw error; }
    socket.on('message',(buffer,remote) => {
      if (buffer.length > 8192 || !this.options.allowPeer(remote.address) || this.pending.size > 64) return;
      const lines = buffer.toString().split('\r\n');
      if (lines[0] !== 'M-SEARCH * HTTP/1.1') return;
      const headers = Object.fromEntries(lines.slice(1).filter(l=>l.includes(':')).map(l=>{const index=l.indexOf(':');return [l.slice(0,index).toLowerCase(),l.slice(index+1).trim()];}));
      if (headers.man?.toLowerCase() !== '"ssdp:discover"') return;
      const requested = targets(this.options.uuid).filter(t=>headers.st === 'ssdp:all' || headers.st === t);
      const mx = Math.min(5,Math.max(1,Number(headers.mx) || 1));
      for (const target of requested) {
        const timer = setTimeout(() => {
          this.pending.delete(timer);
          if (!this.socket) return;
          const reply = discoveryMessage({...this.options,target,response:true});
          this.socket.send(reply,remote.port,remote.address,error=>{if(error)this.lastError=error.message;});
        },Math.random()*mx*1000);
        this.pending.add(timer);
      }
    });
    this.announce(true);
    this.interval = setInterval(()=>this.announce(true),60000);
  }
  announce(alive) {
    if (!this.socket) return;
    for (const target of targets(this.options.uuid)) this.socket.send(discoveryMessage({...this.options,target,alive}),1900,GROUP,error=>{if(error)this.lastError=error.message;});
  }
  async stop() {
    clearInterval(this.interval);
    for (const timer of this.pending) clearTimeout(timer);
    this.pending.clear();
    if (!this.socket) return;
    this.announce(false);
    const socket = this.socket;
    await new Promise(resolve=>setTimeout(resolve,80));
    this.socket = null;
    await new Promise(resolve=>socket.close(resolve));
  }
}
