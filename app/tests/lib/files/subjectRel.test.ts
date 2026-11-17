import { expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { subjectRel } from '../../../src/lib/files/libraryLayout';

// The same cases `course_rel` is held to in
// app/src-tauri/src/library/layout/tests/subject_dirs.rs.
const fixture = JSON.parse(
  readFileSync(join(import.meta.dir, '../../fixtures/library/subject_dirs.json'), 'utf8'),
) as { cases: [string, string][] };

test('every subject code lands in the folder Rust writes it to', () => {
  for (const [code, rel] of fixture.cases) {
    expect([code, subjectRel(code)]).toEqual([code, rel]);
  }
});
