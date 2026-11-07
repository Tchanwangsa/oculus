-- A saved artifact's program is OpenUI Lang or a widget's TSX (docs/widgets.md).
ALTER TABLE revision_artifacts ADD COLUMN format TEXT NOT NULL DEFAULT 'openui'
    CHECK(format IN ('openui','widget'));
