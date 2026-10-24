CREATE TABLE revision_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
    body TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags) AND json_type(tags) = 'array'),
    position REAL NOT NULL DEFAULT 0,
    source_kind TEXT NOT NULL DEFAULT 'manual' CHECK(source_kind IN ('chat','file','lecture','agent','manual')),
    source_ref TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_revision_notes_subject ON revision_notes(subject_id, position);
CREATE TABLE revision_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
    kind TEXT NOT NULL CHECK(kind IN ('card','choice','numeric','short','structured')),
    prompt TEXT NOT NULL CHECK(length(trim(prompt)) > 0),
    answer TEXT NOT NULL CHECK(length(trim(answer)) > 0),
    options TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(options) AND json_type(options) = 'array'),
    tolerance REAL CHECK(tolerance IS NULL OR tolerance >= 0),
    rubric TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(rubric) AND json_type(rubric) = 'array'),
    level TEXT NOT NULL DEFAULT 'medium' CHECK(level IN ('easy','medium','hard')),
    topic TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(topic) AND json_type(topic) = 'array'),
    style_ref TEXT,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','active','suspended')),
    created_by TEXT NOT NULL DEFAULT 'agent' CHECK(created_by IN ('agent','user')),
    due TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    stability REAL NOT NULL DEFAULT 0,
    fsrs_difficulty REAL NOT NULL DEFAULT 0,
    reps INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    state INTEGER NOT NULL DEFAULT 0,
    learning_steps INTEGER NOT NULL DEFAULT 0,
    last_review TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_revision_items_due ON revision_items(status, due, subject_id);
CREATE TABLE revision_sets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE revision_set_items (
    set_id INTEGER NOT NULL REFERENCES revision_sets(id) ON DELETE CASCADE,
    item_id INTEGER NOT NULL REFERENCES revision_items(id) ON DELETE CASCADE,
    position REAL NOT NULL DEFAULT 0,
    PRIMARY KEY(set_id, item_id)
);
CREATE TABLE revision_reviews (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id INTEGER NOT NULL REFERENCES revision_items(id) ON DELETE CASCADE,
    set_id INTEGER REFERENCES revision_sets(id) ON DELETE SET NULL,
    session_id TEXT,
    scheduled INTEGER NOT NULL DEFAULT 1 CHECK(scheduled IN (0,1)),
    at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    answer TEXT NOT NULL DEFAULT '',
    rating INTEGER NOT NULL CHECK(rating BETWEEN 1 AND 4),
    score REAL,
    max REAL,
    feedback TEXT,
    judge_model TEXT,
    overridden INTEGER NOT NULL DEFAULT 0 CHECK(overridden IN (0,1)),
    seconds REAL CHECK(seconds IS NULL OR seconds >= 0),
    working TEXT
);
CREATE INDEX idx_revision_reviews_item ON revision_reviews(item_id, at);
CREATE INDEX idx_revision_reviews_session ON revision_reviews(set_id, session_id);
CREATE TABLE revision_artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
    program TEXT NOT NULL CHECK(length(trim(program)) > 0),
    state TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(state)),
    source_ref TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_revision_artifacts_subject ON revision_artifacts(subject_id);
