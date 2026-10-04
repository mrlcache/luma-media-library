import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { existsSync } from 'node:fs';
import path from 'node:path';

const exec = promisify(execFile);
export class Transcoder {
  constructor(tools) {
    this.ffmpeg = process.env.LUMA_FFMPEG || path.join(tools,'ffmpeg.exe');
    this.ffprobe = process.env.LUMA_FFPROBE || path.join(tools,'ffprobe.exe');
    this.active = new Set(); this.pending = 0; this.generation = 0; this.available = existsSync(this.ffmpeg) && existsSync(this.ffprobe);
    this.lastError = null; this.probes = new Map();
  }
  async plan(file) {
    if (!this.available) throw new Error('FFmpeg is not installed in the server workspace');
    const key = `${file.id}:${file.info.size}:${file.info.mtimeMs}`;
    let data = this.probes.get(key);
    if (!data) {
      const {stdout} = await exec(this.ffprobe,['-v','error','-show_streams','-show_format','-of','json',file.path],{windowsHide:true,timeout:15000,maxBuffer:2*1024*1024});
      data = JSON.parse(stdout); if (this.probes.size >= 1000) this.probes.clear(); this.probes.set(key,data);
    }
    const video = data.streams.find(s=>s.codec_type === 'video');
    if (!video) throw new Error('No video stream');
    const audio = data.streams.find(s=>s.codec_type === 'audio');
    const copyVideo = video.codec_name === 'h264' && ['yuv420p','yuvj420p'].includes(video.pix_fmt);
    const copyAudio = !audio || audio.codec_name === 'aac';
    // Do not silently discard HDR. Tone mapping needs a separate tested profile.
    if (['smpte2084','arib-std-b67'].includes(video.color_transfer) && !copyVideo) throw new Error('HDR conversion is not enabled; use the original file');
    return {mode:copyVideo && copyAudio ? 'remux' : copyVideo ? 'audio-transcode' : 'transcode',copyVideo,copyAudio,duration:Number(data.format?.duration || 0)};
  }
  async stream(file,req,res,offset = 0) {
    if (this.active.size + this.pending >= 2) {res.writeHead(503,{'Retry-After':'5'});res.end('Transcoder busy');return;}
    const generation = this.generation;
    this.pending++;
    let plan;
    try { plan = await this.plan(file); } finally { this.pending--; }
    if (res.destroyed || generation !== this.generation) return;
    const args = ['-nostdin','-hide_banner','-loglevel','error'];
    if (offset > 0) args.push('-ss',String(offset));
    args.push('-i',file.path,'-map','0:v:0','-map','0:a:0?','-sn','-dn');
    if (plan.copyVideo) args.push('-c:v','copy');
    else args.push('-c:v','libx264','-preset','veryfast','-crf','20','-pix_fmt','yuv420p','-vf','scale=trunc(iw/2)*2:trunc(ih/2)*2');
    if (plan.copyAudio) args.push('-c:a','copy');
    else args.push('-c:a','aac','-b:a','192k','-ac','2');
    args.push('-f','mpegts','pipe:1');
    const child = spawn(this.ffmpeg,args,{windowsHide:true,stdio:['ignore','pipe','pipe']});
    this.active.add(child);
    let stderr = '';
    child.stderr.on('data',chunk=>{stderr=(stderr+chunk).slice(-4096);});
    const abort = () => {if(child.exitCode === null) child.kill();};
    res.on('close',abort);
    child.on('error',error=>{this.lastError=error.message;this.active.delete(child);if(!res.headersSent){res.writeHead(502);res.end('Could not start transcoding');}else res.destroy();});
    child.on('close',code=>{this.active.delete(child);res.off('close',abort);if(code && stderr)this.lastError=stderr;if(!res.writableEnded){if(!res.headersSent){res.writeHead(502);res.end('Conversion failed');}else if(code)res.destroy();else res.end();}});
    const headers = {'Content-Type':'video/mp2t','transferMode.dlna.org':'Streaming','contentFeatures.dlna.org':'DLNA.ORG_OP=00;DLNA.ORG_CI=1;DLNA.ORG_FLAGS=01700000000000000000000000000000','X-Luma-Playback':plan.mode,'Cache-Control':'no-store'};
    child.stdout.once('data',chunk=>{if(res.destroyed)return;res.writeHead(200,headers);res.write(chunk);child.stdout.pipe(res);});
  }
  stop() { this.generation++; for (const child of this.active) child.kill(); }
}
