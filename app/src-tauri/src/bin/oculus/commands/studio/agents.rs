//! `oculus studio agents`: each agent's Studio subagents, their models from
//! Settings → Jobs and the call that starts one (`app_lib::harness::studio_agents`).

use crate::*;
use app_lib::harness::{studio_agents, Provider};

const AGENTS: [Provider; 4] = [
    Provider::Claude,
    Provider::Codex,
    Provider::Opencode,
    Provider::Antigravity,
];

impl Ctx {
    pub(crate) fn studio_agents(&self, args: &StudioAgentsArgs) -> Result<(), String> {
        let agents: Vec<Provider> = match &args.agent {
            Some(name) => vec![Provider::parse(name).ok_or_else(|| {
                format!("unknown agent `{name}`: claude, codex, opencode or antigravity")
            })?],
            None => AGENTS.to_vec(),
        };
        let rows: Vec<serde_json::Value> = agents
            .iter()
            .map(|&agent| {
                let roles: Vec<serde_json::Value> = studio_agents::assigned_now(agent)
                    .into_iter()
                    .map(|(role, sel)| {
                        serde_json::json!({
                            "role": role.name,
                            "job": role.job.label(),
                            "model": sel.as_ref().map(|s| s.model.clone()),
                            "effort": sel.as_ref().and_then(|s| s.reasoning_effort.clone()),
                            "spawn": studio_agents::how_to_spawn(agent, role, sel.as_ref()),
                        })
                    })
                    .collect();
                serde_json::json!({ "agent": agent.as_str(), "roles": roles })
            })
            .collect();
        if self.json {
            return self.emit(&rows);
        }
        for row in &rows {
            println!("{}", paint(row["agent"].as_str().unwrap_or(""), BOLD));
            for role in row["roles"].as_array().into_iter().flatten() {
                let model = match (role["model"].as_str(), role["effort"].as_str()) {
                    (Some(m), Some(e)) => format!("{m} ({e})"),
                    (Some(m), None) => m.to_string(),
                    _ => "off".to_string(),
                };
                println!(
                    "  {}  {}",
                    role["role"].as_str().unwrap_or(""),
                    paint(&model, DIM)
                );
                println!("    {}", role["spawn"].as_str().unwrap_or(""));
            }
        }
        Ok(())
    }
}
