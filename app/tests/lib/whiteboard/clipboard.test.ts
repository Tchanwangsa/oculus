import { describe, expect, test } from 'bun:test';
import { cloneWhiteboardElements, parseWhiteboardClipboard, serializeWhiteboardClipboard } from '../../../src/lib/whiteboard/renderClipboard';
import { createWhiteboardDocument, parseWhiteboardDocument } from '../../../src/lib/whiteboard/model';
import type { WhiteboardElement } from '../../../src/lib/whiteboard/model';

const arrow: WhiteboardElement = {
  id: 'original', type: 'arrow', x: -20, y: 30, width: 100, height: 50,
  points: [{ x: 0, y: 0 }, { x: 100, y: 50 }],
  stroke: '#292b32', fill: 'transparent', strokeWidth: 2, style: 'clean', seed: 7,
  futureMetadata: { source: 'lecture-1' },
};

describe('portable whiteboard clipboard', () => {
  test('uses the public document format and preserves element extension fields', () => {
    const encoded = serializeWhiteboardClipboard([arrow]);
    expect(parseWhiteboardDocument(encoded).elements).toEqual([arrow]);
    expect(parseWhiteboardClipboard(encoded)).toEqual([arrow]);
    const publicFile = JSON.stringify({ ...createWhiteboardDocument(), viewport: { x: 100, y: 50, zoom: 2 }, elements: [arrow] });
    expect(parseWhiteboardClipboard(publicFile)).toEqual([arrow]);
  });

  test('rejects ordinary text, malformed envelopes, duplicate IDs, and invalid geometry', () => {
    for (const text of ['hello', '{', '[]', '{}', '{"type":"oculus-whiteboard","version":2}', JSON.stringify({ ...createWhiteboardDocument(), elements: [arrow, arrow] }), JSON.stringify({ ...createWhiteboardDocument(), elements: [{ ...arrow, x: null }] })]) {
      expect(parseWhiteboardClipboard(text)).toBeNull();
    }
    expect(parseWhiteboardClipboard(serializeWhiteboardClipboard([]))).toEqual([]);
  });

  test('pasted and duplicated shapes get fresh IDs and world offsets without moving local points or modifying the source', () => {
    let sequence = 0;
    const copied = cloneWhiteboardElements([arrow, { ...arrow, id: 'second', x: 80 }], { x: 48, y: 48 }, () => `fresh-${++sequence}`);
    expect(copied.map(element => element.id)).toEqual(['fresh-1', 'fresh-2']);
    expect(copied[0]).toMatchObject({ x: 28, y: 78, width: 100, height: 50, points: arrow.points, futureMetadata: arrow.futureMetadata });
    expect(copied[1].x - copied[0].x).toBe(100);
    expect(copied[0].points).not.toBe(arrow.points);
    expect(copied[0].futureMetadata).not.toBe(arrow.futureMetadata);
    expect(arrow).toMatchObject({ id: 'original', x: -20, y: 30 });
  });
});
