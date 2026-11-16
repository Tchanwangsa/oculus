// Wrap the stage 4 steering probes; invoke with the probe asset directory.
// Inbound frames are kept as recorded (home paths and scratch suffixes
// redacted). The probe's own client ids stay in the echo frames; the replay
// substitutes the id Oculus wrote under, which is random per steer.
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
const out = fileURLToPath(new URL("../", import.meta.url));
const source = process.argv[2];
if (!source) throw new Error("Pass assets/probes-2026-11-15");
const sanitize = (v) => JSON.parse(JSON.stringify(v).replaceAll("/Users/tanatchanwangsa", "/Users/student").replace(/(oculus-[^/"\\]*\.)[A-Za-z0-9]{6}/g, "$1XXXXXX"));
const hash = (s) => createHash("sha256").update(s).digest("hex");
const CLIENT = "Replay client";

function save(name, provider, file, range, frames, outbound, settings, scenario) {
  const raw = frames.map((f) => JSON.stringify(sanitize(f))).join("\n") + "\n";
  writeFileSync(`${out}${name}.ndjson`, raw);
  const fixture = {
    provider, cli_version: provider === "codex" ? "0.169.0" : "2.1.325",
    protocol: provider === "codex" ? "app-server JSON-RPC turn/steer" : "print stream-json, user message with priority now and uuid",
    model: provider === "codex" ? "gpt-6.1-sol" : "claude-haiku-5-5", capture_date: "2026-11-15",
    source: `${provider}/${file}.jsonl lines ${range}; inbound frames`,
    source_sha256: hash(readFileSync(`${source}/${provider}/${file}.jsonl`, "utf8")),
    redaction: "Inbound frames as recorded. Home paths become /Users/student; scratch suffixes become XXXXXX. Expected outbound is the Oculus bridge's: its own send envelope, and each steer's write with the client id as \"Replay client\" (Oculus mints a random one); the replay puts that id into the recorded echo frames.",
    scenario, evidence: "recorded",
    frames: frames.length, sha256: hash(raw), expected_outbound: sanitize(outbound), input: `../${name}.ndjson`,
    native_session: provider === "codex" ? frames[0].params.threadId : "", permission_mode: "ask",
    settings,
  };
  writeFileSync(`${out}turn-model/${name}.manifest.json`, JSON.stringify(fixture, null, 2) + "\n");
}

function rows(provider, file) {
  return readFileSync(`${source}/${provider}/${file}.jsonl`, "utf8").trim().split("\n").map(JSON.parse);
}

function codex(file, name, scenario, steers, after = []) {
  const all = rows("codex", file);
  const start = all.findIndex((r) => r.frame.method === "turn/started");
  const end = all.length - 1;
  const selected = all.slice(start, end + 1);
  const frames = selected.filter((r) => r.direction === "server" && r.frame.params?.threadId).map((r) => r.frame);
  const thread = frames[0].params.threadId;
  const turn = frames[0].params.turn.id;
  const template = JSON.parse(readFileSync(`${out}turn-model/codex-approval-command.manifest.json`));
  const send = structuredClone(template.expected_outbound[0]);
  send.frame.params.threadId = thread; send.response.turn.id = turn;
  const outbound = [send];
  const recorded = selected.filter((r) => r.direction === "client" && r.frame.method === "turn/steer");
  steers.forEach((s, i) => {
    const r = recorded[i];
    const reply = selected.find((x) => x.direction === "server" && x.frame.id === r.frame.id).frame;
    const frame = structuredClone(r.frame);
    frame.id = i + 2; frame.params.clientUserMessageId = CLIENT;
    outbound.push(reply.error ? { frame, response: { rpc_error: reply.error }, any_prompt: true } : { frame, response: reply.result, any_prompt: true });
    s.text = r.frame.params.input[0].text;
    s.client = r.frame.params.clientUserMessageId;
  });
  for (const a of after) outbound.push(a(thread, outbound.length + 1));
  save(name, "codex", file, `${start + 1}-${end + 1}`, frames, outbound, { steers }, scenario);
}

function claude(file, name, scenario, steers) {
  const all = rows("claude", file);
  const start = all.findIndex((r) => r.direction === "in");
  const end = all.findLastIndex((r) => r.direction === "in");
  const selected = all.slice(start, end + 1);
  const frames = selected.filter((r) => r.direction === "in").map((r) => r.frame);
  const template = JSON.parse(readFileSync(`${out}turn-model/claude-approval-escape.manifest.json`));
  const outbound = [structuredClone(template.expected_outbound[0])];
  const written = selected.filter((r) => r.direction === "out" && r.frame.priority === "now");
  steers.forEach((s, i) => {
    const frame = structuredClone(written[i].frame);
    s.text = frame.message.content; s.client = frame.uuid;
    frame.uuid = CLIENT;
    outbound.push({ frame, response: null, any_prompt: true });
  });
  save(name, "claude", file, `${start + 1}-${end + 1}`, frames, outbound, { steers }, scenario);
}

const nextTurn = (thread, id) => ({ frame: { jsonrpc: "2.0", id, method: "turn/start", params: { threadId: thread, input: [{ type: "text", text: "Replay request", text_elements: [] }], approvalPolicy: "on-request", approvalsReviewer: "user", sandboxPolicy: { type: "workspaceWrite", writableRoots: [], networkAccess: true, excludeTmpdirEnvVar: false, excludeSlashTmp: false } } }, response: { turn: { id: "replay-next-turn" } }, any_prompt: true });

// The gate is the first inbound frame after the steer was written.
codex("25-steer-tool-lifecycle", "codex-steer-tool", "a steer while a 20 s command runs; the command completes after the turn's terminal", [{ gate: "steer", after: { method: "item/started", type: "commandExecution" } }], [nextTurn]);
codex("21-steer-text-and-finish", "codex-steer-text-finish", "a steer while the answer streams, then one sent at the turn's terminal, which the server refuses", [{ gate: "stream", after_delta: 9 }, { gate: "terminal", missed: true }], [nextTurn]);
claude("11-steer-tool", "claude-steer-tool", "a steer while a 20 s foreground Bash runs: two results, aborted_tools then completed", [{ gate: "steer", before_lifecycle: "queued" }]);
claude("11-steer-text", "claude-steer-text", "a steer while text streams: two results, aborted_streaming then completed", [{ gate: "steer", before_lifecycle: "queued" }]);
claude("11-steer-finish", "claude-steer-finish", "a steer written as the turn's result arrived; the CLI ran it next", [{ gate: "steer", before_result: true }]);
