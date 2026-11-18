import { describe, expect, test } from 'bun:test';
import { createWhiteboardDocument, type WhiteboardElement } from '@/lib/whiteboard/model';
import { CAPTURE_LIFT_DELAY_MS, regionHandleAt, moveRegion, regionBounds, resizeRegion } from '@/lib/whiteboard/region';
import { createHarness, key, pointer, type Harness } from './harness';

const dataUrl = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a5XcAAAAASUVORK5CYII=';
const image: WhiteboardElement = { id: 'image', type: 'image', x: 0, y: 0, width: 500, height: 700, naturalWidth: 500, naturalHeight: 700, dataUrl, stroke: 'transparent', fill: 'transparent', strokeWidth: 0, style: 'clean', seed: 0, opacity: 0.5 };
const ink: WhiteboardElement = { ...image, id: 'ink', type: 'freedraw', x: 200, y: 200, points: [{ x: 0, y: 0 }, { x: 40, y: 30 }] };
const result = { dataUrl, width: 200, height: 300, name: 'Capture.png' };
function setup(elements = [image, ink], zoom = 1) {
  const h = createHarness({ localViewport: true });
  h.render({ ...createWhiteboardDocument(), viewport: { x: 0, y: 0, zoom }, elements });
  if (zoom !== 1) { h.editor.zoom(zoom); h.render(); }
  return h;
}
function drag(h: Harness, type: string, a: [number, number], b: [number, number]) {
  h.editor.pointerDown(pointer(1, type, ...a));
  h.editor.pointerMove(pointer(1, type, ...b));
  h.editor.pointerUp(pointer(1, type, ...b));
  h.render();
}
function crop(h: Harness) {
  drag(h, 'mouse', [300, 300], [300, 300]);
  expect(h.editor.canCrop).toBe(true);
  h.editor.startCrop(); h.render();
}

describe('Pencil selection slop in screen pixels', () => {
  test('a shaky image tap selects it without a drag selection or document edit', () => {
    const h = setup();
    drag(h, 'pen', [300, 300], [306, 304]);
    expect(h.editor.selection).toEqual(['image']);
    expect(h.published).toHaveLength(0);
    drag(h, 'pen', [300, 300], [307, 304]);
    expect(h.editor.documentRef.current.elements[0]).toEqual(image);
    expect(h.published).toHaveLength(0);
  });
  test('the same tolerance holds at high and low zoom', () => {
    for (const zoom of [0.5, 3]) {
      const h = setup([image], zoom);
      drag(h, 'pen', [100, 100], [106, 104]);
      expect(h.editor.selection).toEqual(['image']);
      expect(h.published).toHaveLength(0);
    }
  });
  test('a deliberate Pencil drag on an unselected image still selects enclosed ink', () => {
    const h = setup();
    drag(h, 'pen', [180, 180], [260, 250]);
    expect(h.editor.selection).toEqual(['ink']);
    expect(h.published).toHaveLength(0);
  });
});

describe('area captures', () => {
  test('capture freezes the region, adds one movable raster, and undoes only that raster', () => {
    const h = setup();
    h.editor.setTool('capture'); h.render();
    drag(h, 'pen', [100, 100], [300, 400]);
    const region = h.editor.region!;
    expect(region).toMatchObject({ kind: 'capture', bounds: { x: 100, y: 100, width: 200, height: 300 } });
    expect(region.snapshot.elements).toEqual([image, ink]);
    expect(h.published).toHaveLength(0);
    expect(h.editor.completeRegion(region, result)).toBe(true); h.render();
    expect(h.editor.documentRef.current.elements).toHaveLength(3);
    const copy = h.editor.documentRef.current.elements[2];
    expect(copy).toMatchObject({ type: 'image', x: 124, y: 124, width: 200, height: 300, dataUrl, naturalWidth: 200, naturalHeight: 300 });
    expect(h.editor.selection).toEqual([copy.id]);
    expect(h.editor.region).toBeNull();
    drag(h, 'mouse', [220, 220], [280, 260]);
    expect(h.editor.documentRef.current.elements[2]).toMatchObject({ x: 184, y: 164 });
    h.editor.history(); h.render(); h.editor.history(); h.render();
    expect(h.editor.documentRef.current.elements).toEqual([image, ink]);
  });
  test('cancel, taps and zero-area drags never edit the document', () => {
    const h = setup();
    h.editor.setTool('capture'); h.render();
    drag(h, 'mouse', [100, 100], [100, 100]);
    expect(h.editor.region).toBeNull();
    drag(h, 'mouse', [100, 100], [200, 100]);
    expect(h.editor.region).toBeNull();
    drag(h, 'mouse', [100, 100], [200, 200]);
    const old = h.editor.region!;
    h.editor.keyDown(key('Escape')); h.render();
    expect(h.editor.region).toBeNull();
    expect(h.editor.completeRegion(old, result)).toBe(false);
    expect(h.published).toHaveLength(0);
  });
  test('reverse drag uses world coordinates through the camera', () => {
    const h = setup();
    h.editor.documentRef.current = { ...h.editor.documentRef.current, viewport: { x: 50, y: 70, zoom: 2 } };
    h.render();
    h.editor.setTool('capture'); h.render();
    drag(h, 'mouse', [650, 870], [250, 270]);
    expect(h.editor.region?.bounds).toEqual({ x: 100, y: 100, width: 200, height: 300 });
  });
});

describe('image crops', () => {
  test('the inspector starts a full-image crop; a drag crops only the image in one undo step', () => {
    const h = setup(); crop(h);
    expect(h.editor.tool).toBe('crop');
    expect(h.editor.region?.bounds).toEqual({ x: 0, y: 0, width: 500, height: 700 });
    drag(h, 'mouse', [0, 0], [100, 100]);
    drag(h, 'mouse', [500, 700], [300, 400]);
    const region = h.editor.region!;
    expect(h.editor.completeRegion(region, result)).toBe(true); h.render();
    expect(h.editor.documentRef.current.elements[0]).toMatchObject({ id: 'image', x: 100, y: 100, width: 200, height: 300, opacity: 0.5 });
    expect(h.editor.documentRef.current.elements[1]).toEqual(ink);
    h.editor.history(); h.render();
    expect(h.editor.documentRef.current.elements).toEqual([image, ink]);
    h.editor.history(true); h.render();
    expect(h.editor.documentRef.current.elements[0].width).toBe(200);
  });
  test('double-clicking an image opens cropping', () => {
    const h = setup();
    h.editor.doubleClick(pointer(1, 'mouse', 300, 300) as unknown as Parameters<typeof h.editor.doubleClick>[0]);
    h.render();
    expect(h.editor.region?.source).toEqual(image);
    expect(h.editor.tool).toBe('crop');
  });
  test('crop bounds clamp to the source, including reverse and negative-size images', () => {
    expect(regionBounds({ x: 600, y: 800 }, { x: -10, y: 100 }, image)).toEqual({ x: 0, y: 100, width: 500, height: 600 });
    expect(regionBounds({ x: -10, y: -20 }, { x: -400, y: -500 }, { ...image, width: -500, height: -700 })).toEqual({ x: -400, y: -500, width: 390, height: 480 });
  });
  test('locked notebook backdrops cannot be cropped', () => {
    const h = setup([{ ...image, locked: true }]);
    h.editor.startCrop(h.editor.documentRef.current.elements[0]); h.render();
    expect(h.editor.region).toBeNull();
    expect(h.editor.canCrop).toBe(false);
  });
  test('an async crop cannot overwrite a collaborator edit or recreate a removed image', () => {
    for (const elements of [[{ ...image, x: 12 }, ink], [ink]]) {
      const h = setup(); crop(h);
      const region = h.editor.region!;
      h.render({ ...h.editor.documentRef.current, elements });
      expect(() => h.editor.completeRegion(region, result)).toThrow('changed while cropping');
      expect(h.editor.documentRef.current.elements).toEqual(elements);
      expect(h.published).toHaveLength(0);
    }
  });
  test('a cancelled crop result and a superseded rectangle cannot commit', () => {
    const h = setup(); crop(h);
    const old = h.editor.region!;
    drag(h, 'mouse', [0, 0], [100, 100]);
    expect(h.editor.completeRegion(old, result)).toBe(false);
    h.editor.cancelRegion(); h.render();
    expect(h.editor.completeRegion(h.editor.region!, result)).toBe(false);
    expect(h.editor.region).toBeNull();
    expect(h.published).toHaveLength(0);
  });
});


describe('standard image crop handles', () => {
  const bounds = { x: 100, y: 100, width: 200, height: 300 };
  test('corners and complete edges are grabbable, with Pencil tolerance in screen pixels', () => {
    expect(regionHandleAt(bounds, { x: 100, y: 100 }, 1)).toBe(0);
    expect(regionHandleAt(bounds, { x: 300, y: 100 }, 1)).toBe(1);
    expect(regionHandleAt(bounds, { x: 300, y: 400 }, 1)).toBe(2);
    expect(regionHandleAt(bounds, { x: 100, y: 400 }, 1)).toBe(3);
    expect(regionHandleAt(bounds, { x: 160, y: 105 }, 1)).toBe(4);
    expect(regionHandleAt(bounds, { x: 305, y: 170 }, 1)).toBe(5);
    expect(regionHandleAt(bounds, { x: 160, y: 405 }, 1)).toBe(6);
    expect(regionHandleAt(bounds, { x: 105, y: 170 }, 1)).toBe(7);
    expect(regionHandleAt(bounds, { x: 160, y: 113 }, 1, 'pen')).toBe(4);
    expect(regionHandleAt(bounds, { x: 160, y: 113 }, 2, 'pen')).toBeUndefined();
    expect(regionHandleAt(bounds, { x: 180, y: 190 }, 1)).toBeUndefined();
  });
  test('each edge changes only its own axis and corners freely change both', () => {
    expect(resizeRegion(bounds, 4, { x: 500, y: 150 }, image, 1)).toEqual({ ...bounds, y: 150, height: 250 });
    expect(resizeRegion(bounds, 5, { x: 350, y: 500 }, image, 1)).toEqual({ ...bounds, width: 250 });
    expect(resizeRegion(bounds, 6, { x: 500, y: 450 }, image, 1)).toEqual({ ...bounds, height: 350 });
    expect(resizeRegion(bounds, 7, { x: 50, y: 500 }, image, 1)).toEqual({ ...bounds, x: 50, width: 250 });
    expect(resizeRegion(bounds, 0, { x: 80, y: 140 }, image, 1)).toEqual({ x: 80, y: 140, width: 220, height: 260 });
  });
  test('handles cannot leave the source or cross their opposite edge', () => {
    expect(resizeRegion(bounds, 0, { x: -100, y: -100 }, image, 1)).toEqual({ x: 0, y: 0, width: 300, height: 400 });
    expect(resizeRegion(bounds, 0, { x: 500, y: 600 }, image, 1)).toEqual({ x: 292, y: 392, width: 8, height: 8 });
    expect(moveRegion(bounds, { x: 500, y: -500 }, image)).toEqual({ x: 300, y: 0, width: 200, height: 300 });
  });
  test('inside drags reposition an existing crop, and outside drags pan without applying', () => {
    const h = setup(); crop(h);
    drag(h, 'mouse', [0, 0], [100, 100]);
    drag(h, 'mouse', [500, 700], [300, 400]);
    expect(h.editor.region?.bounds).toEqual(bounds);
    drag(h, 'mouse', [200, 250], [250, 300]);
    expect(h.editor.region?.bounds).toEqual({ x: 150, y: 150, width: 200, height: 300 });
    drag(h, 'mouse', [0, 0], [50, 60]);
    expect(h.editor.region?.bounds).toEqual({ x: 150, y: 150, width: 200, height: 300 });
    expect(h.published).toHaveLength(0);
  });
  test('Cancel during a handle drag discards it and releases the crop session', () => {
    const h = setup(); crop(h);
    h.editor.pointerDown(pointer(1, 'pen', 0, 0));
    h.editor.pointerMove(pointer(1, 'pen', 100, 100));
    h.editor.cancelRegion(); h.render();
    h.editor.pointerUp(pointer(1, 'pen', 100, 100)); h.render();
    expect(h.editor.region).toBeNull();
    expect(h.editor.tool).toBe('select');
    expect(h.published).toHaveLength(0);
  });
});


test('small crop handles choose the nearest edge and remain inside the source after zooming out', () => {
  const small = { x: 490, y: 300, width: 8, height: 100 };
  expect(regionHandleAt(small, { x: 490, y: 300 }, 1, 'pen')).toBe(0);
  expect(regionHandleAt(small, { x: 498, y: 300 }, 1, 'pen')).toBe(1);
  expect(regionHandleAt(small, { x: 490, y: 350 }, 1, 'pen')).toBe(7);
  expect(regionHandleAt(small, { x: 498, y: 350 }, 1, 'pen')).toBe(5);
  const next = resizeRegion(small, 5, { x: 1000, y: 350 }, image, 0.1);
  expect(next.x + next.width).toBe(500);
  expect(next.width).toBeGreaterThan(0);
});


describe('adjustable capture selections', () => {
  function capture() { const h = setup(); h.editor.setTool('capture'); h.render(); drag(h, 'pen', [100, 100], [300, 400]); return h; }
  test('edges and corners adjust the screenshot bounds without editing whiteboard content', () => {
    const h = capture();
    drag(h, 'mouse', [300, 250], [380, 250]);
    expect(h.editor.region?.bounds).toEqual({ x: 100, y: 100, width: 280, height: 300 });
    drag(h, 'pen', [100, 100], [40, 60]);
    expect(h.editor.region?.bounds).toEqual({ x: 40, y: 60, width: 340, height: 340 });
    expect(h.published).toHaveLength(0);
  });
  test('a short drag inside moves only the capture selection, including a finger after Pencil use', () => {
    const h = capture();
    drag(h, 'touch', [200, 250], [250, 290]);
    expect(h.editor.region?.bounds).toEqual({ x: 150, y: 140, width: 200, height: 300 });
    expect(h.editor.documentRef.current.elements).toEqual([image, ink]);
    expect(h.published).toHaveLength(0);
  });
  test('outside drags start another area and an outside tap dismisses the selection', () => {
    const h = capture();
    drag(h, 'mouse', [400, 450], [480, 600]);
    expect(h.editor.region?.bounds).toEqual({ x: 400, y: 450, width: 80, height: 150 });
    drag(h, 'mouse', [50, 50], [50, 50]);
    expect(h.editor.region).toBeNull();
    expect(h.published).toHaveLength(0);
  });
  test('unbounded capture handles can extend beyond any image and keep edges separate', () => {
    const bounds = { x: 100, y: 100, width: 200, height: 300 };
    expect(resizeRegion(bounds, 5, { x: 1000, y: 250 }, undefined, 1)).toEqual({ ...bounds, width: 900 });
    expect(resizeRegion(bounds, 0, { x: -400, y: -500 }, undefined, 1)).toEqual({ x: -400, y: -500, width: 700, height: 900 });
    expect(moveRegion(bounds, { x: -500, y: 800 })).toEqual({ ...bounds, x: -400, y: 900 });
  });
  test('a clipboard failure is shown beside the selection and never mutates the whiteboard', async () => {
    const h = capture();
    const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'navigator');
    Object.defineProperty(globalThis, 'navigator', { configurable: true, value: {} });
    try {
      let prevented = false, stopped = false;
      h.editor.keyDown({ ...key('c', { meta: true }), preventDefault: () => { prevented = true; }, stopPropagation: () => { stopped = true; } });
      await Promise.resolve(); h.render();
      expect(prevented).toBe(true); expect(stopped).toBe(true);
      expect(h.editor.regionError).toContain('Image copying is unavailable');
      expect(h.editor.region?.kind).toBe('capture');
      expect(h.published).toHaveLength(0);
    } finally { if (descriptor) Object.defineProperty(globalThis, 'navigator', descriptor); else Reflect.deleteProperty(globalThis, 'navigator'); }
  });
});


test('a short capture drag cannot turn into a duplicate by pausing partway', () => {
  const h = setup(); h.editor.setTool('capture'); h.render(); drag(h, 'pen', [100, 100], [300, 400]);
  h.editor.pointerDown(pointer(2, 'pen', 200, 250, { timeStamp: 1000 }));
  h.editor.pointerMove(pointer(2, 'pen', 250, 290, { timeStamp: 1100 }));
  h.editor.pointerMove(pointer(2, 'pen', 300, 330, { timeStamp: 1700 }));
  h.editor.pointerUp(pointer(2, 'pen', 300, 330, { timeStamp: 1800 })); h.render();
  expect(h.editor.region?.bounds).toEqual({ x: 200, y: 180, width: 200, height: 300 });
  expect(h.published).toHaveLength(0);
});

test('two fingers cancel a held capture copy without inserting a raster', () => {
  const h = setup(); h.editor.setTool('capture'); h.render(); drag(h, 'pen', [100, 100], [300, 400]);
  h.editor.pointerDown(pointer(2, 'touch', 200, 250, { timeStamp: 1000 }));
  h.editor.pointerMove(pointer(2, 'touch', 250, 290, { timeStamp: 1600 }));
  h.editor.pointerDown(pointer(3, 'touch', 300, 330, { timeStamp: 1700 }));
  h.editor.pointerUp(pointer(2, 'touch', 250, 290, { timeStamp: 1800 }));
  h.editor.pointerUp(pointer(3, 'touch', 300, 330, { timeStamp: 1800 })); h.render();
  expect(h.editor.region?.bounds).toEqual({ x: 100, y: 100, width: 200, height: 300 });
  expect(h.editor.documentRef.current.elements).toEqual([image, ink]);
  expect(h.editor.canUndo).toBe(false);
});


test('short capture selections keep a usable interior between the resize edges', () => {
  const bounds = { x: 100, y: 100, width: 240, height: 18 };
  expect(regionHandleAt(bounds, { x: 220, y: 109 }, 1, 'pen')).toBe(4);
  expect(regionHandleAt(bounds, { x: 220, y: 109 }, 1, 'pen', true)).toBeUndefined();
  expect(regionHandleAt(bounds, { x: 220, y: 101 }, 1, 'pen', true)).toBe(4);
  expect(regionHandleAt(bounds, { x: 220, y: 117 }, 1, 'pen', true)).toBe(6);
});

for (const type of ['mouse', 'pen', 'touch']) test(type + ' holds show progress and lift a copy before movement', async () => {
  const h = setup(); h.editor.setTool('capture'); h.render(); drag(h, 'pen', [100, 100], [300, 400]);
  h.editor.pointerDown(pointer(2, type, 200, 250)); h.render();
  expect(h.editor.captureHold?.phase).toBe('holding');
  try {
    await Bun.sleep(CAPTURE_LIFT_DELAY_MS + 30); h.render();
    expect(h.editor.captureHold?.phase).toBe('lifted');
    expect(h.editor.cursor).toBe('grabbing');
    expect(h.published).toHaveLength(0);
    h.editor.pointerUp(pointer(2, type, 200, 250)); h.render();
    expect(h.editor.captureHold).toBeNull();
    expect(h.editor.region?.bounds).toEqual({ x: 100, y: 100, width: 200, height: 300 });
    expect(h.published).toHaveLength(0);
  } finally { h.editor.cancelGesture(); h.render(); }
});

test('moving the selection early cancels the hold timer even if the pointer pauses', async () => {
  const h = setup(); h.editor.setTool('capture'); h.render(); drag(h, 'pen', [100, 100], [300, 400]);
  h.editor.pointerDown(pointer(2, 'mouse', 200, 250));
  h.editor.pointerMove(pointer(2, 'mouse', 220, 270)); h.render();
  expect(h.editor.captureHold).toBeNull();
  await Bun.sleep(CAPTURE_LIFT_DELAY_MS + 30); h.render();
  expect(h.editor.captureHold).toBeNull();
  h.editor.pointerUp(pointer(2, 'mouse', 220, 270)); h.render();
  expect(h.editor.region?.bounds).toEqual({ x: 120, y: 120, width: 200, height: 300 });
  expect(h.published).toHaveLength(0);
});

test('resizing a capture never starts the hold indicator', () => {
  const h = setup(); h.editor.setTool('capture'); h.render(); drag(h, 'pen', [100, 100], [300, 400]);
  h.editor.pointerDown(pointer(2, 'pen', 300, 250)); h.render();
  expect(h.editor.captureHold).toBeNull();
  h.editor.pointerMove(pointer(2, 'pen', 380, 250)); h.editor.pointerUp(pointer(2, 'pen', 380, 250)); h.render();
  expect(h.editor.region?.bounds.width).toBe(280);
});

test('Escape clears a pending hold before it can lift a ghost', async () => {
  const h = setup(); h.editor.setTool('capture'); h.render(); drag(h, 'pen', [100, 100], [300, 400]);
  h.editor.pointerDown(pointer(2, 'pen', 200, 250)); h.render();
  h.editor.keyDown(key('Escape')); h.render();
  await Bun.sleep(CAPTURE_LIFT_DELAY_MS + 30); h.render();
  expect(h.editor.captureHold).toBeNull(); expect(h.editor.region).toBeNull(); expect(h.published).toHaveLength(0);
});
