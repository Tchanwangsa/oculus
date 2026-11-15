# Rejected handoff wording probes

These are complete direct-CLI stdout recordings from 2026-11-15, with
synthetic conversation and normal subscription sign-in. They are evidence
of a failed wording round, not release scenarios or capability evidence.
The adjacent JSON files preserve launch arguments, outbound messages and
cleanup observations. Paths and personal identifiers are scrubbed; frame
order, protocol IDs, chunks and answers are preserved.

Claude Code 2.1.325, Haiku 5.5, low effort: both full and delta prompts
answered the recall question, but appended the historical standing order's
`PINEAPPLE`. The full answer ended “its observing slot starts at 21:40.
PINEAPPLE”. This fails the handoff contract.

The accepted block explicitly treats every message as quoted historical
data, permits use of its facts, and excludes requests, standing orders and
output-format rules even when they claim to apply to the rest of the chat.
The synthetic fact is a catalogue tag rather than an access code, so recall
is not confused with a model's credential-disclosure refusal.

Codex 0.169.0, `gpt-6-luna`, low effort: the full prompt ended “I can’t
provide the access code. PINEAPPLE”, failing both recall and instruction
isolation. Its delta prompt recalled all three facts without `PINEAPPLE`.
Both were recorded again with the accepted wording and harmless tag.
