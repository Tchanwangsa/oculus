import { describe, expect, test } from 'bun:test';
import { canReorder, reorderElements, type LayerMove } from '../../../src/lib/canvas/layers';
import type { CanvasElement } from '../../../src/lib/canvas/model';

const base = { stroke: '#111111', fill: 'transparent', strokeWidth: 2, style: 'clean' as const, seed: 1 };
const box = (id: string, x: number, y = 0): CanvasElement => ({ id, type: 'rectangle', x, y, width: 100, height: 100, ...base });
const order = (elements: CanvasElement[]) => elements.map(element => element.id).join('');
const move = (elements: CanvasElement[], ids: string, to: LayerMove) => order(reorderElements(elements, new Set(ids), to));

// `stacked` all overlap one another; `apart` overlap nothing.
const stacked = [box('a', 0), box('b', 10), box('c', 20), box('d', 30)];
const apart = [box('a', 0), box('b', 500), box('c', 1000), box('d', 1500)];

describe('reorderElements', () => {
  test('front and back move the selection to the end of the stack, keeping its order', () => {
    expect(move(stacked, 'b', 'front')).toBe('acdb');
    expect(move(stacked, 'c', 'back')).toBe('cabd');
    expect(move(stacked, 'ca', 'front')).toBe('bdac');
    expect(move(stacked, 'db', 'back')).toBe('bdac');
  });

  test('forward and backward step past the nearest overlapping element', () => {
    expect(move(stacked, 'a', 'forward')).toBe('bacd');
    expect(move(stacked, 'd', 'backward')).toBe('abdc');
    const mixed = [box('a', 0), box('far', 800), box('b', 10)];
    expect(move(mixed, 'a', 'forward')).toBe('farba');
    expect(move(mixed, 'b', 'backward')).toBe('bafar');
  });

  test('with nothing overlapping, forward and backward step one place', () => {
    expect(move(apart, 'a', 'forward')).toBe('bacd');
    expect(move(apart, 'c', 'backward')).toBe('acbd');
  });

  test('a multiple selection moves together and keeps its order', () => {
    expect(move(stacked, 'ab', 'forward')).toBe('cabd');
    expect(move(stacked, 'cd', 'backward')).toBe('acdb');
    expect(move(stacked, 'ac', 'forward')).toBe('badc');
    // b overlaps c, a overlaps only d: a steps one place and stays beneath b rather than passing d.
    const split = [box('a', 0), box('b', 1000), box('c', 1010), box('d', 10)];
    expect(move(split, 'ab', 'forward')).toBe('cabd');
  });

  test('a move that changes nothing returns the input array', () => {
    for (const [ids, to] of [['d', 'front'], ['cd', 'forward'], ['a', 'back'], ['ab', 'backward'], ['', 'front']] as const) {
      expect(reorderElements(stacked, new Set(ids), to)).toBe(stacked);
    }
  });

  test('elements keep their identity', () => {
    const next = reorderElements(stacked, new Set(['a']), 'front');
    expect(next).not.toBe(stacked);
    expect(new Set(next)).toEqual(new Set(stacked));
  });
});

describe('canReorder', () => {
  test('is false only when the selection already sits at that end of the stack', () => {
    const ids = (value: string) => new Set(value);
    expect(canReorder(stacked, ids('d'), 'front')).toBe(false);
    expect(canReorder(stacked, ids('cd'), 'forward')).toBe(false);
    expect(canReorder(stacked, ids('bd'), 'forward')).toBe(true);
    expect(canReorder(stacked, ids('ab'), 'back')).toBe(false);
    expect(canReorder(stacked, ids('ac'), 'backward')).toBe(true);
    expect(canReorder(stacked, ids(''), 'front')).toBe(false);
  });

  test('agrees with reorderElements', () => {
    for (const ids of ['a', 'b', 'd', 'ab', 'ad', 'bc', 'cd', 'abcd']) {
      for (const to of ['front', 'forward', 'backward', 'back'] as const) {
        const selection = new Set(ids);
        expect(canReorder(stacked, selection, to)).toBe(reorderElements(stacked, selection, to) !== stacked);
        expect(canReorder(apart, selection, to)).toBe(reorderElements(apart, selection, to) !== apart);
      }
    }
  });
});
