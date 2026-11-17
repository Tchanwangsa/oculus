import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathFile } from '../../../src/lib/files/pathCategory';
import {
  FILES_DIR,
  LESSONS_DIR,
  categoryOf,
  courseRelOf,
  inFolder,
  isOwnPath,
  pagesRecordOf,
  recordingOf,
} from '../../../src/lib/files/libraryLayout';

// The same cases `category_of` is held to in
// app/src-tauri/src/library/layout/tests/category.rs.
const fixture = JSON.parse(
  readFileSync(join(import.meta.dir, '../../fixtures/library/categories.json'), 'utf8'),
) as { cases: [string, string][] };

describe('libraryLayout', () => {
  test('every shared fixture path has the category Rust gives it', () => {
    for (const [path, category] of fixture.cases) {
      expect([path, pathFile(path).category]).toEqual([path, category]);
    }
  });

  test('no prototype key reads as a folder or a synced file', () => {
    expect(categoryOf('Course/constructor')).toBe('other');
    expect(categoryOf('Course/toString/x.md')).toBe('other');
  });

  test('paths inside a subject lose the subject prefix, others are kept', () => {
    expect(courseRelOf('courses/X/Course/Files/a.pdf')).toBe('Course/Files/a.pdf');
    expect(courseRelOf('agents/outputs/a.md')).toBe('agents/outputs/a.md');
    expect(inFolder('courses/X/Course/Lessons/1-l.md', LESSONS_DIR)).toBe(true);
    expect(inFolder('courses/X/Lessons/Course/Files/a.pdf', FILES_DIR)).toBe(false);
  });

  test('own files are everything in a subject beside Course and .oculus', () => {
    expect(isOwnPath('courses/X/Exam prep/notes.md')).toBe(true);
    expect(isOwnPath('courses/X/a.pdf')).toBe(true);
    expect(isOwnPath('courses/X/Course/Files/a.pdf')).toBe(false);
    expect(isOwnPath('courses/X/course/home.md')).toBe(false);
    expect(isOwnPath('courses/X/.oculus/agents/x.md')).toBe(false);
    expect(isOwnPath('agents/outputs/a.md')).toBe(false);
  });

  test('a path in a recording names its folder', () => {
    const folder = 'courses/X/Course/Recordings/2026-08-04-mo-l105';
    expect(recordingOf(`${folder}/frames/live/12-source1.jpg`)).toBe(folder);
    expect(recordingOf(folder)).toBe(folder);
    expect(recordingOf(`${folder}/transcript.vtt#t=12`)).toBe(folder);
    expect(recordingOf('courses/X/Course/Recordings')).toBeNull();
    expect(recordingOf('courses/X/Course/Files/a.pdf')).toBeNull();
  });

  test("a parse record sits under the subject's .oculus/derived, as Rust's layout::derived puts it", () => {
    expect(pagesRecordOf('courses/X/Course/Files/week 1.md')).toBe('courses/X/.oculus/derived/Course/Files/week 1.pages.json');
    expect(pagesRecordOf('courses/X/scan.md')).toBe('courses/X/.oculus/derived/scan.pages.json');
    expect(pagesRecordOf('agents/outputs/a.md')).toBe('agents/outputs/a.pages.json');
  });
});
