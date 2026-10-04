import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { existsSync } from 'node:fs';
import path from 'node:path';

const exec = promisify(execFile);
export const mobileQualities = Object.freeze({
  '480p': { width: 854, height: 480, bitrate: 1_200_000, bitrates: [800_000,1_200_000,2_000_000] },
  '720p': { width: 1280, height: 720, bitrate: 2_500_000, bitrates: [1_500_000,2_500_000,4_000_000] },
  '1080p': { width: 1920, height: 1080, bitrate: 5_000_000, bitrates: [3_000_000,5_000_000,8_000_000] },
});
export class Transcoder {
  constructor(tools) {
    this.ffmpeg = process.env.LUMA_FFMPEG || path.join(tools,'ffmpeg.exe');
    this.ffprobe = process.env.LUMA_FFPROBE || path.join(tools,'ffprobe.exe');
    this.active = new Set(); this.pending = 0; this.generation = 0; this.available = existsSync(this.ffmpeg) && existsSync(this.ffprobe);
    this.lastError = null; this.probes = new Map(); this.sessions = new Map();
  }
  async plan(file,output = 'mpegts',quality = 'auto',bitrate = 0) {
    if (quality !== 'auto' && !Object.hasOwn(mobileQualities,quality)) throw new Error('Unsupported quality');
    if (bitrate !== 0 && !mobileQualities[quality]?.bitrates.includes(bitrate)) throw new Error('Unsupported bitrate for this quality');
    if (!this.available) throw new Error('FFmpeg is not installed in the server workspace');
    const key = `${file.path}:${file.info.size}:${file.info.mtimeMs}`;
    let data = this.probes.get(key);
    if (!data) {
      const {stdout} = await exec(this.ffprobe,['-v','error','-show_streams','-show_format','-of','json',file.path],{windowsHide:true,timeout:15000,maxBuffer:2*1024*1024});
      data = JSON.parse(stdout); if (this.probes.size >= 1000) this.probes.clear(); this.probes.set(key,data);
    }
    const video = data.streams.find(s=>s.codec_type === 'video');
    if (!video) throw new Error('No video stream');
    const audio = data.streams.find(s=>s.codec_type === 'audio');
    const preset = output === 'mp4' ? mobileQualities[quality] : null;
    const profile = preset ? {...preset,bitrate:bitrate || preset.bitrate} : null;
    const copyVideo = !profile && video.codec_name === 'h264' && ['yuv420p','yuvj420p'].includes(video.pix_fmt)
      && (output !== 'mp4' || (Number(video.width) <= 1920 && Number(video.height) <= 1080));
    const copyAudio = !audio || audio.codec_name === 'aac';
    // Do not silently discard HDR. Tone mapping needs a separate tested profile.
    if (['smpte2084','arib-std-b67'].includes(video.color_transfer) && !copyVideo) throw new Error('HDR conversion is not enabled; use the original file');
    return {mode:copyVideo && copyAudio ? 'remux' : copyVideo ? 'audio-transcode' : 'transcode',copyVideo,copyAudio,duration:Number(data.format?.duration || 0),profile:profile || null,sourceBitrate:Number(data.format?.bit_rate || 0)};
  }
  async stream(file,req,res,offset = 0,output = 'mpegts',quality = 'auto',bitrate = 0,session = '') {
    if (!['mpegts','mp4'].includes(output)) throw new Error('Unsupported output format');
    const previous = session ? this.sessions.get(session) : null;
    if (previous?.child && previous.child.exitCode === null) {
      const stopped = new Promise(resolve=>previous.child.once('close',resolve));
      previous.child.kill();
      await Promise.race([stopped,new Promise(resolve=>setTimeout(resolve,1000))]);
    }
    if (this.active.size + this.pending >= 2) {res.writeHead(503,{'Retry-After':'5'});res.end('Transcoder busy');return;}
    const ticket = {child:null};
    if(session)this.sessions.set(session,ticket);
    const releaseSession = () => {if(session && this.sessions.get(session)===ticket)this.sessions.delete(session);};
    const generation = this.generation;
    this.pending++;
    let plan;
    try { plan = await this.plan(file,output,quality,bitrate); } catch(error) {releaseSession();throw error;} finally { this.pending--; }
    if (res.destroyed || generation !== this.generation || (session && this.sessions.get(session)!==ticket)) {releaseSession();res.destroy();return;}
    const args = ['-nostdin','-hide_banner','-loglevel','error'];
    if (offset > 0) args.push('-ss',String(offset));
    args.push('-i',file.path,'-map','0:v:0','-map','0:a:0?','-sn','-dn');
    if (plan.copyVideo) args.push('-c:v','copy');
    else if (output === 'mp4') {
      const profile=plan.profile || mobileQualities['720p'];
      args.push('-c:v','libx264','-preset','veryfast','-tune','zerolatency','-threads','2','-g','48','-crf','23','-pix_fmt','yuv420p','-vf',`scale=w='min(${profile.width},iw)':h='min(${profile.height},ih)':force_original_aspect_ratio=decrease:force_divisible_by=2`);
      if(plan.profile)args.push('-maxrate',String(profile.bitrate),'-bufsize',String(profile.bitrate*2));
    }
    else args.push('-c:v','libx264','-preset','veryfast','-crf','20','-pix_fmt','yuv420p','-vf','scale=trunc(iw/2)*2:trunc(ih/2)*2');
    if (plan.copyAudio) args.push('-c:a','copy');
    else args.push('-c:a','aac','-b:a',output === 'mp4' ? '160k' : '192k','-ac','2');
    if (output === 'mp4') args.push('-movflags','frag_keyframe+empty_moov+default_base_moof','-frag_duration','1000000','-f','mp4','pipe:1');
    else args.push('-f','mpegts','pipe:1');
    const child = spawn(this.ffmpeg,args,{windowsHide:true,stdio:['ignore','pipe','pipe']});
    ticket.child = child;
    this.active.add(child);
    let stderr = '';
    child.stderr.on('data',chunk=>{stderr=(stderr+chunk).slice(-4096);});
    const abort = () => {if(child.exitCode === null) child.kill();};
    res.on('close',abort);
    child.on('error',error=>{releaseSession();this.lastError=error.message;this.active.delete(child);if(!res.headersSent){res.writeHead(502);res.end('Could not start transcoding');}else res.destroy();});
    child.on('close',code=>{releaseSession();this.active.delete(child);res.off('close',abort);if(code && stderr)this.lastError=stderr;if(!res.writableEnded){if(!res.headersSent){res.writeHead(502);res.end('Conversion failed');}else if(code)res.destroy();else res.end();}});
    const headers = {'Content-Type':output === 'mp4' ? 'video/mp4' : 'video/mp2t','X-Luma-Duration':String(plan.duration),'X-Luma-Playback':plan.mode,'Cache-Control':'no-store'};
    if (output === 'mpegts') Object.assign(headers,{'transferMode.dlna.org':'Streaming','contentFeatures.dlna.org':'DLNA.ORG_OP=00;DLNA.ORG_CI=1;DLNA.ORG_FLAGS=01700000000000000000000000000000'});
    else Object.assign(headers,{'Access-Control-Allow-Origin':'*','Access-Control-Expose-Headers':'X-Luma-Duration'});
    child.stdout.once('data',chunk=>{if(res.destroyed)return;res.writeHead(200,headers);res.write(chunk);child.stdout.pipe(res);});
  }
  stop() { this.generation++; for (const child of this.active) child.kill(); }
}
