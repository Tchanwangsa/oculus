CREATE TABLE revision_tags (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL CHECK(length(trim(name)) > 0),
    color      TEXT    NOT NULL,
    position   REAL    NOT NULL DEFAULT 0,
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX idx_revision_tags_name ON revision_tags(name COLLATE NOCASE);

INSERT INTO revision_tags (name, color, position) VALUES
    ('Definition', 'indigo', 1),
    ('Formula', 'purple', 2),
    ('Theorem', 'rose', 3),
    ('Method', 'green', 4),
    ('Worked example', 'amber', 5),
    ('Note', 'slate', 6);

INSERT OR IGNORE INTO revision_tags (name, color, position)
SELECT trim(j.value), 'slate',
       (SELECT MAX(position) FROM revision_tags) + ROW_NUMBER() OVER (ORDER BY n.id, j.key)
  FROM revision_notes n, json_each(n.tags) j
 WHERE j.type = 'text' AND trim(j.value) <> ''
 ORDER BY n.id, j.key;

UPDATE revision_notes
   SET tags = (SELECT json_group_array(COALESCE(
                   (SELECT g.name FROM revision_tags g WHERE g.name = trim(j.value) COLLATE NOCASE),
                   trim(j.value)))
                 FROM json_each(revision_notes.tags) j
                WHERE j.type = 'text' AND trim(j.value) <> ''
                  AND NOT EXISTS (SELECT 1 FROM json_each(revision_notes.tags) k
                                   WHERE k.key < j.key AND trim(k.value) = trim(j.value) COLLATE NOCASE));

-- Every writer registers a new name, then the entry takes the registry's
-- spelling with duplicates dropped. The WHEN stops at a canonical list.
CREATE TRIGGER revision_note_tags_insert AFTER INSERT ON revision_notes
WHEN EXISTS (SELECT 1 FROM json_each(NEW.tags) j
              WHERE j.type <> 'text'
                 OR NOT EXISTS (SELECT 1 FROM revision_tags g WHERE g.name = j.value)
                 OR EXISTS (SELECT 1 FROM json_each(NEW.tags) k
                             WHERE k.key < j.key AND k.value = j.value COLLATE NOCASE))
BEGIN
    INSERT OR IGNORE INTO revision_tags (name, color, position)
    SELECT trim(value), 'slate',
           (SELECT COALESCE(MAX(position), 0) FROM revision_tags) + CAST(key AS INTEGER) + 1
      FROM json_each(NEW.tags) WHERE type = 'text' AND trim(value) <> '';
    UPDATE revision_notes
       SET tags = (SELECT json_group_array(COALESCE(
                       (SELECT g.name FROM revision_tags g WHERE g.name = trim(j.value) COLLATE NOCASE),
                       trim(j.value)))
                     FROM json_each(NEW.tags) j
                    WHERE j.type = 'text' AND trim(j.value) <> ''
                      AND NOT EXISTS (SELECT 1 FROM json_each(NEW.tags) k
                                       WHERE k.key < j.key AND trim(k.value) = trim(j.value) COLLATE NOCASE))
     WHERE id = NEW.id;
END;

CREATE TRIGGER revision_note_tags_update AFTER UPDATE OF tags ON revision_notes
WHEN EXISTS (SELECT 1 FROM json_each(NEW.tags) j
              WHERE j.type <> 'text'
                 OR NOT EXISTS (SELECT 1 FROM revision_tags g WHERE g.name = j.value)
                 OR EXISTS (SELECT 1 FROM json_each(NEW.tags) k
                             WHERE k.key < j.key AND k.value = j.value COLLATE NOCASE))
BEGIN
    INSERT OR IGNORE INTO revision_tags (name, color, position)
    SELECT trim(value), 'slate',
           (SELECT COALESCE(MAX(position), 0) FROM revision_tags) + CAST(key AS INTEGER) + 1
      FROM json_each(NEW.tags) WHERE type = 'text' AND trim(value) <> '';
    UPDATE revision_notes
       SET tags = (SELECT json_group_array(COALESCE(
                       (SELECT g.name FROM revision_tags g WHERE g.name = trim(j.value) COLLATE NOCASE),
                       trim(j.value)))
                     FROM json_each(NEW.tags) j
                    WHERE j.type = 'text' AND trim(j.value) <> ''
                      AND NOT EXISTS (SELECT 1 FROM json_each(NEW.tags) k
                                       WHERE k.key < j.key AND trim(k.value) = trim(j.value) COLLATE NOCASE))
     WHERE id = NEW.id;
END;

CREATE TRIGGER revision_tag_rename AFTER UPDATE OF name ON revision_tags
BEGIN
    UPDATE revision_notes
       SET tags = (SELECT json_group_array(CASE WHEN value = OLD.name COLLATE NOCASE
                                              THEN NEW.name ELSE value END)
                     FROM json_each(revision_notes.tags))
     WHERE EXISTS (SELECT 1 FROM json_each(revision_notes.tags) WHERE value = OLD.name COLLATE NOCASE);
END;

CREATE TRIGGER revision_tag_delete BEFORE DELETE ON revision_tags
BEGIN
    UPDATE revision_notes
       SET tags = (SELECT json_group_array(value) FROM json_each(revision_notes.tags)
                    WHERE value <> OLD.name COLLATE NOCASE)
     WHERE EXISTS (SELECT 1 FROM json_each(revision_notes.tags) WHERE value = OLD.name COLLATE NOCASE);
END;
