CREATE TABLE revision_topics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id INTEGER REFERENCES subjects(id) ON DELETE CASCADE,
    name TEXT NOT NULL COLLATE NOCASE CHECK(length(trim(name)) > 0),
    description TEXT NOT NULL DEFAULT '',
    position REAL NOT NULL DEFAULT 0,
    is_unsorted INTEGER NOT NULL DEFAULT 0 CHECK(is_unsorted IN (0,1)),
    CHECK(is_unsorted = 0 OR name = 'Unsorted')
);
CREATE UNIQUE INDEX idx_revision_topics_name ON revision_topics(COALESCE(subject_id,-1),name COLLATE NOCASE);
CREATE UNIQUE INDEX idx_revision_topics_unsorted ON revision_topics(COALESCE(subject_id,-1)) WHERE is_unsorted=1;
INSERT INTO revision_topics(subject_id,name,is_unsorted) VALUES(NULL,'Unsorted',1);
INSERT INTO revision_topics(subject_id,name,is_unsorted) SELECT id,'Unsorted',1 FROM subjects;
CREATE TABLE revision_note_topics (
    note_id INTEGER NOT NULL REFERENCES revision_notes(id) ON DELETE CASCADE,
    topic_id INTEGER NOT NULL REFERENCES revision_topics(id) ON DELETE CASCADE,
    PRIMARY KEY(note_id,topic_id)
);
CREATE INDEX idx_revision_note_topics_topic ON revision_note_topics(topic_id);
CREATE TABLE revision_item_topics (
    item_id INTEGER NOT NULL REFERENCES revision_items(id) ON DELETE CASCADE,
    topic_id INTEGER NOT NULL REFERENCES revision_topics(id) ON DELETE CASCADE,
    PRIMARY KEY(item_id,topic_id)
);
CREATE INDEX idx_revision_item_topics_topic ON revision_item_topics(topic_id);
CREATE TABLE revision_set_topics (
    set_id INTEGER NOT NULL REFERENCES revision_sets(id) ON DELETE CASCADE,
    topic_id INTEGER NOT NULL REFERENCES revision_topics(id) ON DELETE CASCADE,
    PRIMARY KEY(set_id,topic_id)
);
CREATE INDEX idx_revision_set_topics_topic ON revision_set_topics(topic_id);
CREATE TABLE revision_artifact_topics (
    artifact_id INTEGER NOT NULL REFERENCES revision_artifacts(id) ON DELETE CASCADE,
    topic_id INTEGER NOT NULL REFERENCES revision_topics(id) ON DELETE CASCADE,
    PRIMARY KEY(artifact_id,topic_id)
);
CREATE INDEX idx_revision_artifact_topics_topic ON revision_artifact_topics(topic_id);
INSERT OR IGNORE INTO revision_topics(subject_id,name)
SELECT r.subject_id,trim(j.value) FROM revision_notes r,json_each(r.tags) j
WHERE j.type='text' AND length(trim(j.value))>0;
INSERT OR IGNORE INTO revision_note_topics(note_id,topic_id)
SELECT r.id,t.id FROM revision_notes r,json_each(r.tags) j
JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.name=trim(j.value) COLLATE NOCASE
WHERE j.type='text' AND length(trim(j.value))>0 AND t.is_unsorted=0;
INSERT OR IGNORE INTO revision_topics(subject_id,name)
SELECT r.subject_id,trim(j.value) FROM revision_items r,json_each(r.topic) j
WHERE j.type='text' AND length(trim(j.value))>0;
INSERT OR IGNORE INTO revision_item_topics(item_id,topic_id)
SELECT r.id,t.id FROM revision_items r,json_each(r.topic) j
JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.name=trim(j.value) COLLATE NOCASE
WHERE j.type='text' AND length(trim(j.value))>0 AND t.is_unsorted=0;
INSERT INTO revision_note_topics(note_id,topic_id)
SELECT r.id,t.id FROM revision_notes r JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.is_unsorted=1
WHERE NOT EXISTS(SELECT 1 FROM revision_note_topics m WHERE m.note_id=r.id);
INSERT INTO revision_item_topics(item_id,topic_id)
SELECT r.id,t.id FROM revision_items r JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.is_unsorted=1
WHERE NOT EXISTS(SELECT 1 FROM revision_item_topics m WHERE m.item_id=r.id);
INSERT INTO revision_set_topics(set_id,topic_id)
SELECT r.id,t.id FROM revision_sets r JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.is_unsorted=1
WHERE NOT EXISTS(SELECT 1 FROM revision_set_topics m WHERE m.set_id=r.id);
INSERT INTO revision_artifact_topics(artifact_id,topic_id)
SELECT r.id,t.id FROM revision_artifacts r JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.is_unsorted=1
WHERE NOT EXISTS(SELECT 1 FROM revision_artifact_topics m WHERE m.artifact_id=r.id);
CREATE TRIGGER revision_topics_subject_immutable BEFORE UPDATE OF subject_id ON revision_topics
WHEN NEW.subject_id IS NOT OLD.subject_id BEGIN SELECT RAISE(ABORT,'A topic cannot change subjects'); END;
CREATE TRIGGER revision_topics_unsorted_immutable BEFORE UPDATE ON revision_topics
WHEN OLD.is_unsorted=1 AND (NEW.name IS NOT OLD.name OR NEW.is_unsorted<>1)
BEGIN SELECT RAISE(ABORT,'Unsorted cannot be renamed'); END;
CREATE TRIGGER revision_topics_unsorted_delete BEFORE DELETE ON revision_topics
WHEN OLD.is_unsorted=1 AND (OLD.subject_id IS NULL OR EXISTS(SELECT 1 FROM subjects WHERE id=OLD.subject_id))
BEGIN SELECT RAISE(ABORT,'Unsorted cannot be deleted'); END;
CREATE TRIGGER revision_topics_subject_insert AFTER INSERT ON subjects
BEGIN INSERT INTO revision_topics(subject_id,name,is_unsorted) VALUES(NEW.id,'Unsorted',1); END;
CREATE TRIGGER revision_note_topic_default AFTER INSERT ON revision_notes
BEGIN
 INSERT INTO revision_note_topics(note_id,topic_id)
 SELECT NEW.id,id FROM revision_topics WHERE subject_id IS NEW.subject_id AND is_unsorted=1;
END;
CREATE TRIGGER revision_note_topic_scope AFTER UPDATE OF subject_id ON revision_notes
WHEN NEW.subject_id IS NOT OLD.subject_id
BEGIN
 DELETE FROM revision_note_topics WHERE note_id=NEW.id;
 INSERT OR IGNORE INTO revision_note_topics(note_id,topic_id)
 SELECT NEW.id,id FROM revision_topics WHERE subject_id IS NEW.subject_id AND is_unsorted=1;
END;
CREATE TRIGGER revision_note_topic_validate BEFORE INSERT ON revision_note_topics
WHEN NOT EXISTS(SELECT 1 FROM revision_notes r JOIN revision_topics t ON t.subject_id IS r.subject_id WHERE r.id=NEW.note_id AND t.id=NEW.topic_id)
BEGIN SELECT RAISE(ABORT,'Topic must belong to the same subject'); END;
CREATE TRIGGER revision_note_topic_update_validate BEFORE UPDATE ON revision_note_topics
BEGIN SELECT RAISE(ABORT,'Replace topic memberships using delete and insert'); END;
CREATE TRIGGER revision_note_topic_named AFTER INSERT ON revision_note_topics
BEGIN
 DELETE FROM revision_note_topics WHERE note_id=NEW.note_id
 AND topic_id IN(SELECT id FROM revision_topics WHERE is_unsorted=1)
 AND EXISTS(SELECT 1 FROM revision_note_topics m JOIN revision_topics t ON t.id=m.topic_id WHERE m.note_id=NEW.note_id AND t.is_unsorted=0);
END;
CREATE TRIGGER revision_note_topic_fallback AFTER DELETE ON revision_note_topics
WHEN NOT EXISTS(SELECT 1 FROM revision_note_topics WHERE note_id=OLD.note_id)
BEGIN
 INSERT OR IGNORE INTO revision_note_topics(note_id,topic_id)
 SELECT r.id,t.id FROM revision_notes r JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.is_unsorted=1 WHERE r.id=OLD.note_id;
END;
CREATE TRIGGER revision_item_topic_default AFTER INSERT ON revision_items
BEGIN
 INSERT INTO revision_item_topics(item_id,topic_id)
 SELECT NEW.id,id FROM revision_topics WHERE subject_id IS NEW.subject_id AND is_unsorted=1;
END;
CREATE TRIGGER revision_item_topic_scope AFTER UPDATE OF subject_id ON revision_items
WHEN NEW.subject_id IS NOT OLD.subject_id
BEGIN
 DELETE FROM revision_item_topics WHERE item_id=NEW.id;
 INSERT OR IGNORE INTO revision_item_topics(item_id,topic_id)
 SELECT NEW.id,id FROM revision_topics WHERE subject_id IS NEW.subject_id AND is_unsorted=1;
END;
CREATE TRIGGER revision_item_topic_validate BEFORE INSERT ON revision_item_topics
WHEN NOT EXISTS(SELECT 1 FROM revision_items r JOIN revision_topics t ON t.subject_id IS r.subject_id WHERE r.id=NEW.item_id AND t.id=NEW.topic_id)
BEGIN SELECT RAISE(ABORT,'Topic must belong to the same subject'); END;
CREATE TRIGGER revision_item_topic_update_validate BEFORE UPDATE ON revision_item_topics
BEGIN SELECT RAISE(ABORT,'Replace topic memberships using delete and insert'); END;
CREATE TRIGGER revision_item_topic_named AFTER INSERT ON revision_item_topics
BEGIN
 DELETE FROM revision_item_topics WHERE item_id=NEW.item_id
 AND topic_id IN(SELECT id FROM revision_topics WHERE is_unsorted=1)
 AND EXISTS(SELECT 1 FROM revision_item_topics m JOIN revision_topics t ON t.id=m.topic_id WHERE m.item_id=NEW.item_id AND t.is_unsorted=0);
END;
CREATE TRIGGER revision_item_topic_fallback AFTER DELETE ON revision_item_topics
WHEN NOT EXISTS(SELECT 1 FROM revision_item_topics WHERE item_id=OLD.item_id)
BEGIN
 INSERT OR IGNORE INTO revision_item_topics(item_id,topic_id)
 SELECT r.id,t.id FROM revision_items r JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.is_unsorted=1 WHERE r.id=OLD.item_id;
END;
CREATE TRIGGER revision_set_topic_default AFTER INSERT ON revision_sets
BEGIN
 INSERT INTO revision_set_topics(set_id,topic_id)
 SELECT NEW.id,id FROM revision_topics WHERE subject_id IS NEW.subject_id AND is_unsorted=1;
END;
CREATE TRIGGER revision_set_topic_scope AFTER UPDATE OF subject_id ON revision_sets
WHEN NEW.subject_id IS NOT OLD.subject_id
BEGIN
 DELETE FROM revision_set_topics WHERE set_id=NEW.id;
 INSERT OR IGNORE INTO revision_set_topics(set_id,topic_id)
 SELECT NEW.id,id FROM revision_topics WHERE subject_id IS NEW.subject_id AND is_unsorted=1;
END;
CREATE TRIGGER revision_set_topic_validate BEFORE INSERT ON revision_set_topics
WHEN NOT EXISTS(SELECT 1 FROM revision_sets r JOIN revision_topics t ON t.subject_id IS r.subject_id WHERE r.id=NEW.set_id AND t.id=NEW.topic_id)
BEGIN SELECT RAISE(ABORT,'Topic must belong to the same subject'); END;
CREATE TRIGGER revision_set_topic_update_validate BEFORE UPDATE ON revision_set_topics
BEGIN SELECT RAISE(ABORT,'Replace topic memberships using delete and insert'); END;
CREATE TRIGGER revision_set_topic_named AFTER INSERT ON revision_set_topics
BEGIN
 DELETE FROM revision_set_topics WHERE set_id=NEW.set_id
 AND topic_id IN(SELECT id FROM revision_topics WHERE is_unsorted=1)
 AND EXISTS(SELECT 1 FROM revision_set_topics m JOIN revision_topics t ON t.id=m.topic_id WHERE m.set_id=NEW.set_id AND t.is_unsorted=0);
END;
CREATE TRIGGER revision_set_topic_fallback AFTER DELETE ON revision_set_topics
WHEN NOT EXISTS(SELECT 1 FROM revision_set_topics WHERE set_id=OLD.set_id)
BEGIN
 INSERT OR IGNORE INTO revision_set_topics(set_id,topic_id)
 SELECT r.id,t.id FROM revision_sets r JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.is_unsorted=1 WHERE r.id=OLD.set_id;
END;
CREATE TRIGGER revision_artifact_topic_default AFTER INSERT ON revision_artifacts
BEGIN
 INSERT INTO revision_artifact_topics(artifact_id,topic_id)
 SELECT NEW.id,id FROM revision_topics WHERE subject_id IS NEW.subject_id AND is_unsorted=1;
END;
CREATE TRIGGER revision_artifact_topic_scope AFTER UPDATE OF subject_id ON revision_artifacts
WHEN NEW.subject_id IS NOT OLD.subject_id
BEGIN
 DELETE FROM revision_artifact_topics WHERE artifact_id=NEW.id;
 INSERT OR IGNORE INTO revision_artifact_topics(artifact_id,topic_id)
 SELECT NEW.id,id FROM revision_topics WHERE subject_id IS NEW.subject_id AND is_unsorted=1;
END;
CREATE TRIGGER revision_artifact_topic_validate BEFORE INSERT ON revision_artifact_topics
WHEN NOT EXISTS(SELECT 1 FROM revision_artifacts r JOIN revision_topics t ON t.subject_id IS r.subject_id WHERE r.id=NEW.artifact_id AND t.id=NEW.topic_id)
BEGIN SELECT RAISE(ABORT,'Topic must belong to the same subject'); END;
CREATE TRIGGER revision_artifact_topic_update_validate BEFORE UPDATE ON revision_artifact_topics
BEGIN SELECT RAISE(ABORT,'Replace topic memberships using delete and insert'); END;
CREATE TRIGGER revision_artifact_topic_named AFTER INSERT ON revision_artifact_topics
BEGIN
 DELETE FROM revision_artifact_topics WHERE artifact_id=NEW.artifact_id
 AND topic_id IN(SELECT id FROM revision_topics WHERE is_unsorted=1)
 AND EXISTS(SELECT 1 FROM revision_artifact_topics m JOIN revision_topics t ON t.id=m.topic_id WHERE m.artifact_id=NEW.artifact_id AND t.is_unsorted=0);
END;
CREATE TRIGGER revision_artifact_topic_fallback AFTER DELETE ON revision_artifact_topics
WHEN NOT EXISTS(SELECT 1 FROM revision_artifact_topics WHERE artifact_id=OLD.artifact_id)
BEGIN
 INSERT OR IGNORE INTO revision_artifact_topics(artifact_id,topic_id)
 SELECT r.id,t.id FROM revision_artifacts r JOIN revision_topics t ON t.subject_id IS r.subject_id AND t.is_unsorted=1 WHERE r.id=OLD.artifact_id;
END;
CREATE TRIGGER revision_topics_subject_delete BEFORE DELETE ON subjects BEGIN
 UPDATE revision_notes SET subject_id=NULL WHERE subject_id=OLD.id;
 UPDATE revision_items SET subject_id=NULL WHERE subject_id=OLD.id;
 UPDATE revision_sets SET subject_id=NULL WHERE subject_id=OLD.id;
 UPDATE revision_artifacts SET subject_id=NULL WHERE subject_id=OLD.id;
END;
CREATE TABLE agent_changes_topics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    thread_id INTEGER NOT NULL,
    proposal_id TEXT NOT NULL,
    entity TEXT NOT NULL CHECK(entity IN ('project','task','tag','event','note','cheat','question','quiz','artifact','topic')),
    entity_id TEXT NOT NULL,
    op TEXT NOT NULL CHECK(op IN ('create','update','move','refile','delete')),
    before TEXT,
    after TEXT,
    at TEXT NOT NULL DEFAULT (datetime('now')),
    reverted_at TEXT
);
INSERT INTO agent_changes_topics SELECT * FROM agent_changes;
UPDATE sqlite_sequence SET seq=MAX(seq,COALESCE((SELECT seq FROM sqlite_sequence WHERE name='agent_changes'),0)) WHERE name='agent_changes_topics';
DROP TABLE agent_changes;
ALTER TABLE agent_changes_topics RENAME TO agent_changes;
CREATE INDEX idx_agent_changes_proposal ON agent_changes(proposal_id,id);
UPDATE agent_changes SET after=json_set(after,'$.topic_ids',json((SELECT json_group_array(topic_id) FROM (SELECT topic_id FROM revision_note_topics WHERE note_id=CAST(agent_changes.entity_id AS INTEGER) ORDER BY topic_id)))) WHERE entity='cheat' AND after IS NOT NULL;
UPDATE agent_changes SET before=json_set(before,'$.topic_ids',json((SELECT json_group_array(id) FROM revision_topics WHERE is_unsorted=1 AND subject_id IS json_extract(agent_changes.before,'$.row.subject_id')))) WHERE entity='cheat' AND before IS NOT NULL;
CREATE TRIGGER revision_topics_membership_delete AFTER DELETE ON revision_topics
WHEN OLD.is_unsorted=0
BEGIN
 DELETE FROM revision_note_topics WHERE topic_id=OLD.id;
 DELETE FROM revision_item_topics WHERE topic_id=OLD.id;
 DELETE FROM revision_set_topics WHERE topic_id=OLD.id;
 DELETE FROM revision_artifact_topics WHERE topic_id=OLD.id;
END;
