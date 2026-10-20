import { expect, test } from 'bun:test';
import { installBrowserSelectionGuard, isCanvasTextEditorTarget, preventCanvasChromeSelection, preventNativeCanvasTouch } from '../../../src/lib/canvas/browserSelectionGuard';

// A minimal selector port keeps these policy tests independent of a browser runtime.
function element(kind: string, editable?: string, parent?: Element): Element {
  const result = {
    nodeType: 1,
    tagName: kind.toUpperCase(),
    parentElement: parent ?? null,
    getAttribute: (name: string) => name === 'contenteditable' ? editable ?? null : null,
    closest(selector: string): Element | null {
      if (selector === 'input,textarea' && ['input', 'textarea'].includes(kind)) return result as unknown as Element;
      if (selector === '[contenteditable]' && editable !== undefined) return result as unknown as Element;
      return parent?.closest(selector) ?? null;
    },
  };
  return result as unknown as Element;
}

function selectionEvent(target: EventTarget, type = 'selectstart'): Event {
  const event = new Event(type, { cancelable: true });
  Object.defineProperty(event, 'target', { value: target });
  return event;
}

test('canvas chrome selection and menus are cancelled without cancelling editor selection', () => {
  for (const type of ['selectstart', 'contextmenu']) {
    for (const target of [element('canvas'), element('footer'), element('button')]) {
      const event = selectionEvent(target, type);
      preventCanvasChromeSelection(event);
      expect(event.defaultPrevented).toBe(true);
    }
    for (const target of [element('textarea'), element('input'), element('span', undefined, element('div', 'true'))]) {
      const event = selectionEvent(target, type);
      preventCanvasChromeSelection(event);
      expect(event.defaultPrevented).toBe(false);
    }
  }
});

test('text node targets, plaintext hosts and explicitly noneditable islands are classified correctly', () => {
  const textInEditor = { nodeType: 3, parentElement: element('div', '') } as unknown as EventTarget;
  const textInFooter = { nodeType: 3, parentElement: element('footer') } as unknown as EventTarget;
  expect(isCanvasTextEditorTarget(textInEditor)).toBe(true);
  expect(isCanvasTextEditorTarget(textInFooter)).toBe(false);
  expect(isCanvasTextEditorTarget(element('div', 'plaintext-only'))).toBe(true);
  expect(isCanvasTextEditorTarget(element('span', undefined, element('div', 'false', element('div', 'true'))))).toBe(false);
  expect(isCanvasTextEditorTarget(null)).toBe(false);
});

test('native touch defaults are blocked only on the canvas while events keep propagating', () => {
  for (const type of ['touchstart', 'touchend']) {
    for (const [target, blocked] of [[element('canvas'), true], [element('button'), false], [element('textarea'), false], [element('input'), false], [element('a'), false], [element('canvas', undefined, element('div', 'true')), false]] as const) {
      const event = selectionEvent(target, type);
      preventNativeCanvasTouch(event);
      expect(event.defaultPrevented).toBe(blocked);
      expect(event.cancelBubble).toBe(false);
    }
  }
});

test('installation uses nonpassive canvas touch listeners and cleans up without pointer listeners', () => {
  const added: string[] = [], removed: string[] = [];
  const options = new Map<string, unknown>();
  const root = {
    addEventListener: (type: string, _handler: unknown, config: unknown) => { added.push(type); options.set(type, config); },
    removeEventListener: (type: string) => removed.push(type),
  } as unknown as HTMLElement;
  const cleanup = installBrowserSelectionGuard(root);
  expect(added).toEqual(['selectstart', 'contextmenu', 'touchstart', 'touchend']);
  expect(options.get('touchstart')).toEqual({ capture: true, passive: false });
  expect(options.get('touchend')).toEqual({ capture: true, passive: false });
  cleanup();
  expect(removed).toEqual(added);
});
