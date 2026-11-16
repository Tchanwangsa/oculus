// Wrap the stage 3 wire probes; invoke with the probe asset directory.
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
const out = fileURLToPath(new URL("../", import.meta.url));
const source = process.argv[2];
if (!source) throw new Error("Pass assets/probes-2026-11-15");
const clone = (v) => JSON.parse(JSON.stringify(v));
const sanitize = (v) => JSON.parse(JSON.stringify(v).replaceAll("/Users/tanatchanwangsa", "/Users/student").replace(/(oculus-[^/"\\]*\.)[A-Za-z0-9]{6}/g, "$1XXXXXX"));
const hash = (s) => createHash("sha256").update(s).digest("hex");
function save(name, provider, rows, frames, outbound, answers, derived, stop = false) {
  const raw = frames.map((f) => JSON.stringify(sanitize(f))).join("\n") + "\n";
  writeFileSync(`${out}${name}.ndjson`, raw);
  const fixture = {
    provider, cli_version: provider === "codex" ? "0.169.0" : "2.1.325",
    protocol: provider === "codex" ? "app-server JSON-RPC experimental requestUserInput" : "print stream-json host permission prompts",
    model: provider === "codex" ? "gpt-6.1-sol" : "claude-haiku-5-5", capture_date: "2026-11-15",
    source: rows.source, source_sha256: rows.hash,
    redaction: "Inbound turn frames only. Home paths become /Users/student; scratch suffixes become XXXXXX. Expected outbound uses the production bridge's send envelope. " + derived,
    scenario: name, evidence: derived ? "derived, not recorded" : "recorded",
    frames: frames.length, sha256: hash(raw), expected_outbound: sanitize(outbound), input: `../${name}.ndjson`,
    native_session: provider === "codex" ? outbound[0].frame.params.threadId : "", permission_mode: "ask",
    settings: { question_answers: answers, stop, derivation: derived },
  };
  writeFileSync(`${out}turn-model/${name}.manifest.json`, JSON.stringify(fixture, null, 2) + "\n");
}
function wrap(provider, file, suffix) {
  const raw = readFileSync(`${source}/${provider}/${file}.jsonl`, "utf8");
  const all = raw.trim().split("\n").map(JSON.parse);
  const inbound = provider === "codex" ? "server" : "in";
  const stop = file.includes("withdraw");
  const start = provider === "codex" ? all.findIndex((r) => r.frame.method === "turn/started") : 2;
  const end = all.findLastIndex((r) => r.frame.method === "turn/completed" || r.frame.type === "result");
  const selected = all.slice(start, end + 1 + (provider === "codex" && stop ? 1 : 0));
  const frames = selected.filter((r) => r.direction === inbound && r.frame && (provider !== "codex" || r.frame.params?.threadId)).map((r) => clone(r.frame));
  const answers = [];
  let outbound;
  let derived = "";
  if (provider === "codex") {
    const thread = frames[0].params.threadId;
    const turn = frames.find((f) => f.method === "turn/started").params.turn.id;
    const template = JSON.parse(readFileSync(`${out}turn-model/codex-approval-command.manifest.json`));
    const send = clone(template.expected_outbound[0]);
    send.frame.params.threadId = thread; send.response.turn.id = turn;
    outbound = [send];
    for (const f of frames.filter((f) => f.method === "item/tool/requestUserInput")) {
      for (const q of f.params.questions) if (q.isSecret === "[REDACTED]") { q.isSecret = false; derived = "Derived, not recorded: the probe redacted isSecret to a string; false is substituted as a non-secret test input, not evidence of its recorded value."; }
    }
    for (const r of selected.filter((r) => r.direction === "client" && r.frame.result?.answers)) {
      const a = r.frame;
      outbound.push({ frame: { jsonrpc: "2.0", ...a }, response: null });
      answers.push({ native: JSON.stringify(a.id), option: "answer", answers: Object.entries(a.result.answers).map(([questionId, a]) => ({ questionId, answers: a.answers, dismissed: false })) });
    }
    if (stop) outbound.push({frame:{jsonrpc:"2.0",id:2,method:"turn/interrupt",params:{threadId:thread,turnId:turn}},response:{}});
  } else {
    const template = JSON.parse(readFileSync(`${out}turn-model/claude-approval-escape.manifest.json`));
    outbound = [clone(template.expected_outbound[0])];
    for (const r of selected.filter((r) => r.direction === "out" && r.frame.type === "control_response")) {
      const f = r.frame; outbound.push({frame:clone(f),response:null});
      const a = f.response.response.updatedInput.answers;
      const questions = f.response.response.updatedInput.questions;
      answers.push({native:JSON.stringify(f.response.request_id), option:"answer",answers:Object.entries(a).map(([questionId,text])=>({questionId,answers:questions.find((q)=>q.question===questionId).multiSelect ? text.split(", ") : [text],dismissed:false}))});
    }
    if (stop) {
      outbound.push({frame:{type:"control_request",request_id:"oculus-1",request:{subtype:"interrupt"}},response:null});
      for (const f of frames) if (f.type === "control_response") f.response.request_id = "oculus-1";
    }
  }
  const metadata = { source: `${provider}/${file}.jsonl lines ${start+1}-${end+1}; inbound frames`, hash: hash(raw) };
  save(`${provider}-question-${suffix}`,provider,metadata,frames,outbound,answers,derived,stop);
  return {provider,metadata,frames,outbound,answers,derived};
}
const single = wrap("codex","10-question-single","single");
wrap("codex","13-question-two","two");
wrap("codex","14-question-nonblocking","nonblocking");
wrap("codex","15-question-withdraw","withdraw");
wrap("codex","24-question-plan-blocking","typed-blocking");
const claude = wrap("claude","09-question-single","single");
wrap("claude","09-question-multi","multi");
wrap("claude","09-question-free","typed");
wrap("claude","09-question-two","two");
wrap("claude","10-question-withdraw","withdraw");
// Dismissal was not recorded. Keep only the recorded prefix through the
// request and derive an empty answer/deny. EOF proves session cleanup only.
for (const base of [single,claude]) {
 const frames = clone(base.frames);
 const at = frames.findIndex((f)=> f.method === "item/tool/requestUserInput" || f.type === "control_request");
 const f = frames[at];
 const native = JSON.stringify(base.provider === "codex" ? f.id : f.request_id);
 const answer = base.provider === "codex" ? {jsonrpc:"2.0",id:f.id,result:{answers:{}}} : {type:"control_response",response:{subtype:"success",request_id:f.request_id,response:{behavior:"deny",message:"The student declined this. Do not retry it or try another way to do the same thing."}}};
 save(`${base.provider}-question-dismiss`,base.provider,base.metadata,frames.slice(0,at+1),[base.outbound[0],{frame:answer,response:null}],[{native,option:"dismiss",answers:[]}],"Derived, not recorded: recorded prefix through its question; empty answer/deny and transport EOF are synthetic. " + base.derived);
}
