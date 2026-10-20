INSERT OR IGNORE INTO task_tags (name, color, position)
SELECT j.value, 'slate',
       (SELECT COALESCE(MAX(position), 0) FROM task_tags) + ROW_NUMBER() OVER (ORDER BY p.id, j.key)
  FROM projects p, json_each(CASE WHEN json_valid(p.tags) THEN p.tags ELSE '[]' END) j
 WHERE j.type = 'text' AND trim(j.value) <> ''
 ORDER BY p.id, j.key;

CREATE TRIGGER project_tags_insert AFTER INSERT ON projects
BEGIN
    INSERT OR IGNORE INTO task_tags (name, color, position)
    SELECT value, 'slate',
           (SELECT COALESCE(MAX(position), 0) FROM task_tags) + CAST(key AS INTEGER) + 1
      FROM json_each(NEW.tags) WHERE type = 'text' AND trim(value) <> '';
END;

CREATE TRIGGER project_tags_update AFTER UPDATE OF tags ON projects
BEGIN
    INSERT OR IGNORE INTO task_tags (name, color, position)
    SELECT value, 'slate',
           (SELECT COALESCE(MAX(position), 0) FROM task_tags) + CAST(key AS INTEGER) + 1
      FROM json_each(NEW.tags) WHERE type = 'text' AND trim(value) <> '';
END;

CREATE TRIGGER shared_tag_rename AFTER UPDATE OF name ON task_tags
BEGIN
    UPDATE projects
       SET tags = (SELECT json_group_array(CASE WHEN value = OLD.name COLLATE NOCASE
                                              THEN NEW.name ELSE value END)
                     FROM json_each(projects.tags)),
           updated_at = datetime('now')
     WHERE EXISTS (SELECT 1 FROM json_each(projects.tags) WHERE value = OLD.name COLLATE NOCASE);
END;

CREATE TRIGGER shared_tag_delete BEFORE DELETE ON task_tags
BEGIN
    UPDATE projects
       SET tags = (SELECT json_group_array(value) FROM json_each(projects.tags)
                    WHERE value <> OLD.name COLLATE NOCASE),
           updated_at = datetime('now')
     WHERE EXISTS (SELECT 1 FROM json_each(projects.tags) WHERE value = OLD.name COLLATE NOCASE);
END;
