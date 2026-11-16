CREATE VIRTUAL TABLE harness_items_fts USING fts5(
    content,
    content='harness_items',
    content_rowid='id',
    tokenize="unicode61 remove_diacritics 2"
);
CREATE TRIGGER harness_items_fts_ai AFTER INSERT ON harness_items
WHEN new.kind IN ('user', 'assistant') BEGIN
    INSERT INTO harness_items_fts(rowid, content) VALUES (new.id, new.content);
END;
CREATE TRIGGER harness_items_fts_ad AFTER DELETE ON harness_items
WHEN old.kind IN ('user', 'assistant') BEGIN
    INSERT INTO harness_items_fts(harness_items_fts, rowid, content)
    VALUES ('delete', old.id, old.content);
END;
CREATE TRIGGER harness_items_fts_au AFTER UPDATE OF id, content, kind ON harness_items
WHEN old.id IS NOT new.id OR old.content IS NOT new.content OR old.kind IS NOT new.kind BEGIN
    INSERT INTO harness_items_fts(harness_items_fts, rowid, content)
    SELECT 'delete', old.id, old.content WHERE old.kind IN ('user', 'assistant');
    INSERT INTO harness_items_fts(rowid, content)
    SELECT new.id, new.content WHERE new.kind IN ('user', 'assistant');
END;
INSERT INTO harness_items_fts(rowid, content)
SELECT id, content FROM harness_items WHERE kind IN ('user', 'assistant');
