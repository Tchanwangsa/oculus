use serde_json::Value;

use super::fixture::Fixture;

#[derive(Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Mutation {
    #[default]
    None,
    EditedFinal,
    AnswerBeforeTool,
    RepeatSnapshot,
    MissingTerminal,
    PublicNarration,
    Artifact,
    PartialOnly,
    CodexPhase,
    UnknownReasoning,
    AgyRefusal,
    IdenticalDistinct,
    RepeatedTerminal,
    Compaction,
    AfterSendBeforeAck,
}

pub(super) struct Scenario {
    pub fixture: Fixture,
    pub frames: Vec<Value>,
    pub fail_write: bool,
    pub fail_intent: bool,
    pub repeat_envelope: bool,
    pub load_race: bool,
    pub restart: bool,
    /// The run is a card's continuation, started by its `CardSettled`, not a send.
    pub continuation: bool,
}

impl Scenario {
    pub fn recorded(name: &str) -> Self {
        let (fixture, frames) = Fixture::load(name);
        Self {
            fixture,
            frames,
            fail_write: false,
            fail_intent: false,
            repeat_envelope: false,
            load_race: false,
            restart: false,
            continuation: false,
        }
    }

    pub fn mutate(mut self, mutation: Mutation) -> Self {
        let last = self.frames.iter().rposition(|v| {
            v["type"] == "assistant"
                && v["message"]["content"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|b| b["type"] == "text"))
        });
        match mutation {
            Mutation::None => {}
            Mutation::EditedFinal => {
                self.frames[last.unwrap()]["message"]["content"][0]["text"]
                    .as_str()
                    .unwrap();
                let text = &mut self.frames[last.unwrap()]["message"]["content"][0]["text"];
                *text = Value::String(format!("{} Edited.", text.as_str().unwrap()));
            }
            Mutation::AnswerBeforeTool => {
                // Keep the first text message and trailing tool frames; omit later public text.
                let mut seen = false;
                self.frames.retain(|v| {
                    let public = v["type"] == "assistant"
                        && v["message"]["content"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|b| b["type"] == "text"));
                    if !public {
                        return true;
                    }
                    let keep = !seen;
                    seen = true;
                    keep
                });
            }
            Mutation::RepeatSnapshot => {
                let index = match self.fixture.provider {
                    crate::harness::Provider::Claude => last.unwrap(),
                    crate::harness::Provider::Codex => self
                        .frames
                        .iter()
                        .rposition(|v| {
                            v["method"] == "item/completed"
                                && v["params"]["item"]["type"] == "agentMessage"
                        })
                        .unwrap(),
                    crate::harness::Provider::Opencode => self
                        .frames
                        .iter()
                        .rposition(|v| {
                            v["type"] == "message.part.updated"
                                && v["properties"]["part"]["type"] == "text"
                                && !v["properties"]["part"]["time"]["end"].is_null()
                        })
                        .unwrap(),
                    crate::harness::Provider::Antigravity => self
                        .frames
                        .iter()
                        .rposition(|v| {
                            v["event"] == "step_update"
                                && v["step_update"]["state"] == "DONE"
                                && v["step_update"]["step_type"] == "agent_response"
                        })
                        .unwrap(),
                };
                self.frames.insert(index + 1, self.frames[index].clone());
            }
            Mutation::MissingTerminal => {
                self.frames.retain(|v| {
                    v["type"] != "result"
                        && v["method"] != "turn/completed"
                        && v["event"] != "result"
                });
            }
            Mutation::PublicNarration => {
                // Explicit mutation, not evidence of the fixed CLI's protocol.
                let frame = self
                    .frames
                    .iter_mut()
                    .find(|v| {
                        v["type"] == "assistant" && v["message"]["content"][0]["type"] == "thinking"
                    })
                    .unwrap();
                frame["message"]["content"][0]["thinking"] =
                    Value::String("Public summary mutation".into());
            }
            Mutation::AfterSendBeforeAck => {
                self.fixture.expected_outbound[0].error =
                    Some("replay: transport lost after recorded write".into());
                self.frames.clear();
            }
            Mutation::PartialOnly => {
                let index = self
                    .frames
                    .iter()
                    .rposition(|v| {
                        v["method"] == "item/agentMessage/delta"
                            || v["event"]["delta"]["type"] == "text_delta"
                    })
                    .unwrap();
                self.frames.truncate(index + 1);
            }
            Mutation::CodexPhase => {
                let mut seen = false;
                for frame in &mut self.frames {
                    if frame["method"] == "item/completed"
                        && frame["params"]["item"]["type"] == "agentMessage"
                    {
                        frame["params"]["item"]["phase"] =
                            if seen { "final_answer" } else { "commentary" }.into();
                        seen = true;
                    }
                }
                let index = self
                    .frames
                    .iter()
                    .rposition(|v| v["method"] == "item/completed")
                    .unwrap();
                let mut plan = self.frames[index].clone();
                plan["params"]["item"] = serde_json::json!({"id":"mutation-plan", "type":"plan", "text":"Plan mutation"});
                self.frames.insert(index, plan);
            }
            Mutation::UnknownReasoning => {
                self.frames.retain(|v| {
                    !(v["type"] == "message.part.updated"
                        && v["properties"]["part"]["type"] == "reasoning")
                });
            }
            Mutation::AgyRefusal => {
                let result = self
                    .frames
                    .iter_mut()
                    .find(|v| v["event"] == "result")
                    .unwrap();
                result["result"]["denied_actions"] =
                    serde_json::json!([{"action":"write_file", "display_name":"WriteFile"}]);
            }
            Mutation::IdenticalDistinct => {
                let index = last.unwrap();
                let mut other = self.frames[index].clone();
                other["message"]["id"] = "mutation-distinct-message".into();
                other["uuid"] = "mutation-distinct-frame".into();
                self.frames.insert(index + 1, other);
            }
            Mutation::RepeatedTerminal => {
                let index = self
                    .frames
                    .iter()
                    .rposition(|v| {
                        v["type"] == "result"
                            || v["method"] == "turn/completed"
                            || v["event"] == "result"
                    })
                    .unwrap();
                self.frames.insert(index + 1, self.frames[index].clone());
            }
            Mutation::Compaction => {
                for frame in &mut self.frames {
                    if frame["method"] == "item/completed" {
                        frame["params"]["item"] = serde_json::json!({"id":"mutation-compaction", "type":"contextCompaction"});
                    }
                }
                self.frames
                    .retain(|v| v["method"] != "item/agentMessage/delta");
            }
            Mutation::Artifact => {
                let frame = self
                    .frames
                    .iter_mut()
                    .find(|v| {
                        v["type"] == "assistant" && v["message"]["content"][0]["type"] == "text"
                    })
                    .unwrap();
                frame["message"]["content"][0]["text"] = Value::String("```openui\nroot = TextContent(\"Study\")\n```\n```widget\nexport default () => <div>Quiz</div>\n```".into());
            }
        }
        self
    }
}
