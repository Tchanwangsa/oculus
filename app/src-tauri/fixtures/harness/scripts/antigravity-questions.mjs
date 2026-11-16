// Wrap the recorded ACP question exchange and explicitly label synthetic cases.
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
const out = fileURLToPath(new URL("../", import.meta.url));
const source = process.argv[2];
if (!source) throw new Error("Pass assets/probes-2026-11-15/antigravity");
const raw = readFileSync(`${source}/05-questions.jsonl`, "utf8");
const rows = raw.trim().split("\n").map(JSON.parse);
const start = rows.findIndex((r) => r.direction === "client_to_agent" && r.frame.method === "session/prompt");
const sent = rows[start].frame;
const end = rows.findIndex((r, i) => i > start && r.direction === "agent_to_client" && r.frame.id === sent.id && r.frame.result);
const frames = rows.slice(start + 1, end + 1).filter((r) => r.direction === "agent_to_client").map((r) => r.frame);
const at = frames.findIndex((f) => f.method === "elicitation/create");
const question = frames[at];
const response = rows.find((r, i) => i > start && r.direction === "client_to_agent" && r.frame.id === question.id).frame;
const clone = (v) => structuredClone(v);
const hash = (s) => createHash("sha256").update(s).digest("hex");
const prompt = clone(sent); prompt.params.prompt = [{type:"text", text:"Replay request"}];
const expected = (frame) => ({frame,response:null});
function save(suffix, input, answer, answers, derivation, stop = false) {
  const name = `antigravity-question-${suffix}`;
  const text = input.map((f) => JSON.stringify(f).replaceAll("/Users/tanatchanwangsa", "/Users/student")).join("\n") + "\n";
  writeFileSync(`${out}${name}.ndjson`, text);
  const outbound = [expected(prompt)];
  if (stop) outbound.push(expected({jsonrpc:"2.0",method:"session/cancel",params:{sessionId:sent.params.sessionId}}));
  outbound.push(expected(answer));
  const manifest = {
    provider:"antigravity", cli_version:"1.5.9", protocol:"Agent Client Protocol v1 elicitation/create forms over agy acp",
    model:"gemini-3.8-flash-low", capture_date:"2026-11-15",
    source:`antigravity/05-questions.jsonl lines ${start+1}-${end+1}; inbound frames and exact recorded accept reply`,
    source_sha256:hash(raw), evidence:derivation ? "derived, not recorded" : "recorded",
    redaction:"Home paths become /Users/student. Replay starts at the first prompt; initialization is tested separately. " + derivation,
    scenario:name, frames:input.length, sha256:hash(text), input:`../${name}.ndjson`,
    expected_outbound:outbound, native_session:sent.params.sessionId, permission_mode:"ask",
    settings:{question_answers:answers,stop,derivation},
  };
  writeFileSync(`${out}turn-model/${name}.manifest.json`, JSON.stringify(manifest,null,2)+"\n");
}
const qa = (questionId, answers, dismissed = false) => ({questionId,answers,dismissed});
const reply = (result) => ({jsonrpc:"2.0",id:question.id,result});
const a = (option, answers) => [{native:JSON.stringify(question.id),option,answers}];
save("single",frames,response,a("answer",[qa("q1",["Blue"])]),"");
const prefix = () => clone(frames.slice(0, at+1));
const terminal = (stopReason) => ({jsonrpc:"2.0",id:sent.id,result:{stopReason}});
const typed = prefix(); typed.push(terminal("end_turn"));
save("typed",typed,reply({action:"accept",content:{q1_other:"Green"}}),a("answer",[qa("q1",["Green"])]),"Derived, not recorded: recorded prefix, typed Other answer and synthetic terminal; no live typed continuation is claimed.");
const multi = prefix();
const properties = multi.at(-1).params.requestedSchema.properties;
properties.q1.type = "array"; properties.q1.items = {type:"string",oneOf:properties.q1.oneOf}; delete properties.q1.oneOf;
properties.q2 = {type:"string",title:"Continue?",description:"Should we continue?",oneOf:[{const:"yes",title:"Yes"},{const:"no",title:"No"}]};
multi.push(terminal("end_turn"));
save("multi-two",multi,reply({action:"accept",content:{q1:["1","2"],q1_other:"(Recommended) Red, Blue, Green",q2:"yes"}}),a("answer",[qa("q1",["(Recommended) Red","Blue","Green"]),qa("q2",["Yes"])]),"Derived, not recorded: recorded prefix with array items.oneOf and a second choice-only property, mixed selected/typed answers and a synthetic terminal. No live multi-select or multi-question behavior is claimed.");
save("dismiss",prefix(),reply({action:"decline"}),a("dismiss",[]),"Derived, not recorded: recorded prefix, decline and transport EOF; no live dismissal continuation is claimed.");
const stopped = prefix(); stopped.push(terminal("cancelled"));
save("stop",stopped,reply({action:"decline"}),[],"Derived, not recorded: recorded prefix, session/cancel, decline and cancelled terminal; no live question-cancellation ordering is claimed.",true);
