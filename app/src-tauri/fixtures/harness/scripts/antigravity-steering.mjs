// Recorded cancel episodes plus explicitly derived restart/race traffic.
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
const source = process.argv[2];
if (!source) throw new Error('Pass assets/probes-2026-11-15/antigravity');
const raw = readFileSync(`${source}/07-cancel.jsonl`, 'utf8');
const rows = raw.trim().split('\n').map(JSON.parse);
const inbound = (first, last) => rows.slice(first - 1, last).filter(r => r.direction === 'agent_to_client').map(r => structuredClone(r.frame));
const root = fileURLToPath(new URL('../', import.meta.url));
const hash = s => createHash('sha256').update(s).digest('hex');
const sid = rows[8].frame.params.sessionId;
const correction = 'Reply AFTER_CANCEL_OK only. No tools.';
const request = (id, text) => ({jsonrpc:'2.0', id, method:'session/prompt', params:{sessionId:sid, prompt:[{type:'text',text}]}});
const cancel = {jsonrpc:'2.0',method:'session/cancel',params:{sessionId:sid}};
const after = id => inbound(37, 41).map(f => f.id === 7 ? {...f,id} : f);
function save(suffix, frames, writes, actions, derivation) {
  const name = `antigravity-steer-${suffix}`;
  const text = frames.map(f => JSON.stringify(f).replaceAll('/Users/tanatchanwangsa','/Users/student')).join('\n')+'\n';
  writeFileSync(`${root}${name}.ndjson`, text);
  const manifest = {
    provider:'antigravity',cli_version:'1.5.9',protocol:'Agent Client Protocol v1 over agy acp',model:'gemini-3.8-flash-low',capture_date:'2026-11-15',
    source:'antigravity/07-cancel.jsonl: command cancellation lines 10-23, text cancellation lines 27-31, successful same-session continuation lines 37-41',source_sha256:hash(raw),
    evidence:'derived, not recorded',redaction:`Recorded inbound segments with home redaction; ${derivation}`,scenario:name,
    frames:frames.length,sha256:hash(text),input:`../${name}.ndjson`,native_session:sid,permission_mode:'ask',
    expected_outbound:writes.map(frame => ({frame,response:null})),settings:{actions,derivation},
  };
  writeFileSync(`${root}turn-model/${name}.manifest.json`,JSON.stringify(manifest,null,2)+'\n');
}
const text = inbound(27,31);
const terminal = text.length - 1;
const steer = (text=correction) => ({before:terminal,kind:'steer',text});
const prompt = request(5,'Replay request');
const normal = 'derived, not recorded: the successful continuation is spliced immediately after cancellation, its prompt reply id changes from 7 to 6, and its user echo matches the correction. Other probe episodes and the unsupported session/steer call are omitted; no live restart-steer timing is claimed.';
save('text',[...text,...after(6)],[prompt,cancel,request(6,correction)],[steer()],normal);
const tool = inbound(10,23);
const allow = rows[18].frame;
save('tool',[...tool,...after(5)],[request(4,'Replay request'),allow,cancel,request(5,correction)],[{before:tool.length-2,kind:'steer',text:correction}],normal.replace('7 to 6','7 to 5'));
const two = 'First correction.\n\nSecond correction.';
const twoAfter = after(6); twoAfter[0].params.update.content.text = two;
save('two',[...text,...twoAfter],[prompt,cancel,request(6,two)],[steer('First correction.'),steer('Second correction.')],normal+' Two pending corrections and the replacement user echo are synthetic.');
const missed = structuredClone(text); missed.at(-1).result.stopReason='end_turn';
save('missed',[...missed,...after(6)],[prompt,cancel,request(6,correction)],[steer()],normal+' The first stopReason is synthetically end_turn: cancel lost the race.');
save('stop',text,[prompt,cancel,cancel],[steer(),{before:terminal,kind:'stop'}],'derived, not recorded: Stop follows a pending restart steer before the recorded cancelled terminal. No live Stop/restart race is claimed.');
save('during-stop',[...text,...after(6)],[prompt,cancel,request(6,correction)],[{before:terminal,kind:'stop'},steer()],'derived, not recorded: the correction is submitted after Stop and before the recorded cancelled terminal; it queues as an ordinary new message and starts after the stopped terminal, as the runtime send rule requires.');
save('death',text.slice(0,-1),[prompt,cancel],[{before:terminal-1,kind:'steer',text:correction}],'derived, not recorded: transport EOF replaces the cancelled terminal after the cancel write; no live process death was recorded.');
const error = structuredClone(text); error[terminal]={jsonrpc:'2.0',id:5,error:{code:-32603,message:'Derived prompt failure'}};
save('error',error,[prompt,cancel],[steer()],'derived, not recorded: a synthetic RPC error replaces the cancelled terminal after the cancel write; errors do not authorize restart.');
const longCorrection = rows[25].frame.params.prompt[0].text;
save('successive',[...tool,...text,...after(6)],[request(4,'Replay request'),allow,cancel,request(5,longCorrection),cancel,request(6,correction)],[{before:tool.length-2,kind:'steer',text:longCorrection},{before:tool.length+terminal,kind:'steer',text:correction}],normal+' Both recorded cancellations remain in sequence; two separate restart attempts are driven by the two original next-prompt texts.');
