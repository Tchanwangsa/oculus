import { describe, expect, test } from 'bun:test';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { parseCanvasDocument, serializeCanvasDocument } from '../../../src/lib/canvas/model';
import { applyCanvasTransaction, sameValue } from '../../../src/lib/canvas/collaboration';

// The same files drive the Rust authority's tests (`canvases/authority/tests/`),
// so the reference implementation and the native port cannot drift apart.
const FIXTURES = join(import.meta.dir, '../../fixtures');
const MODEL = join(FIXTURES, 'canvas-model');
const COLLAB = join(FIXTURES, 'canvas-collab');
const files = (dir: string) => readdirSync(dir).filter(name => name.endsWith('.json')).sort();

/** The error codes the authority reports, from the reference implementation's messages. */
function errorCode(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes('same shape')) return 'conflict-shape';
  if (message.includes('same layer order')) return 'conflict-order';
  if (message.includes('changed the background') || message.includes('changed the pages')) return 'conflict-background';
  return 'invalid';
}

describe('canvas model fixtures', () => {
  for (const name of files(join(MODEL, 'valid'))) {
    test(`accepts ${name} and serializes it canonically`, () => {
      const text = readFileSync(join(MODEL, 'valid', name), 'utf8');
      const canonical = join(MODEL, 'serialized', name);
      expect(serializeCanvasDocument(parseCanvasDocument(text))).toBe(existsSync(canonical) ? readFileSync(canonical, 'utf8') : text);
    });
  }
  for (const name of files(join(MODEL, 'invalid'))) {
    test(`rejects ${name}`, () => {
      expect(() => parseCanvasDocument(readFileSync(join(MODEL, 'invalid', name), 'utf8'))).toThrow();
    });
  }
});

describe('canvas transaction fixtures', () => {
  for (const name of files(COLLAB)) {
    const fixture = JSON.parse(readFileSync(join(COLLAB, name), 'utf8'));
    test(`${name}: ${fixture.name}`, () => {
      const document = parseCanvasDocument(JSON.stringify(fixture.document));
      let outcome: { ok: unknown } | { error: string };
      try { outcome = { ok: applyCanvasTransaction(document, fixture.transaction) }; } catch (error) { outcome = { error: errorCode(error) }; }
      if ('error' in fixture.expect) {
        expect(outcome).toEqual({ error: fixture.expect.error });
      } else {
        expect('ok' in outcome).toBe(true);
        const result = (outcome as { ok: unknown }).ok;
        expect(sameValue(result, fixture.expect.ok)).toBe(true);
        expect(Object.keys(result as object)).toEqual(Object.keys(fixture.document));
      }
    });
  }
});
