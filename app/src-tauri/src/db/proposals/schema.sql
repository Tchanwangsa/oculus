CREATE TABLE chat_proposals (
    item_id INTEGER NOT NULL,
    block INTEGER NOT NULL CHECK (block >= 0),
    thread_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending','created','discarded','superseded','undone')),
    state TEXT NOT NULL CHECK (json_valid(state)),
    initial_state TEXT NOT NULL CHECK (json_valid(initial_state)),
    told TEXT,
    superseded_by TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (item_id, block)
);
CREATE INDEX idx_chat_proposals_thread ON chat_proposals(thread_id, item_id, block);

CREATE TABLE agent_changes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    thread_id INTEGER NOT NULL,
    proposal_id TEXT NOT NULL,
    entity TEXT NOT NULL CHECK (entity IN ('project','task','tag','event','note','cheat')),
    entity_id TEXT NOT NULL,
    op TEXT NOT NULL CHECK (op IN ('create','update','move','refile','delete')),
    before TEXT,
    after TEXT,
    at TEXT NOT NULL DEFAULT (datetime('now')),
    reverted_at TEXT
);
CREATE INDEX idx_agent_changes_proposal ON agent_changes(proposal_id, id);
