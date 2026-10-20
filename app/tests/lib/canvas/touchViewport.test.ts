import { describe, expect, test } from 'bun:test';
import { TouchViewportGesture } from '../../../src/lib/canvas/touchViewport';

const initial = { x: 20, y: 40, zoom: 2 };
const start = () => {
  const gesture = new TouchViewportGesture();
  expect(gesture.down(1, { x: 100, y: 100 }, initial)).toBe(false);
  expect(gesture.down(2, { x: 200, y: 100 }, initial)).toBe(true);
  return gesture;
};
describe('two-finger canvas navigation', () => {
  test('translates without changing scale when both fingers move together', () => {
    const gesture = start();
    gesture.move(1, { x: 130, y: 150 });
    expect(gesture.move(2, { x: 230, y: 150 })).toEqual({ x: 50, y: 90, zoom: 2 });
  });
  test('pinches around the world point between the fingers', () => {
    const gesture = start();
    gesture.move(1, { x: 50, y: 100 });
    const viewport = gesture.move(2, { x: 250, y: 100 })!;
    expect(viewport.zoom).toBe(4);
    expect((150 - viewport.x) / viewport.zoom).toBe((150 - initial.x) / initial.zoom);
    expect((100 - viewport.y) / viewport.zoom).toBe((100 - initial.y) / initial.zoom);
  });
  test('combines pan and zoom in one gesture', () => {
    const gesture = start();
    gesture.move(1, { x: 80, y: 150 });
    expect(gesture.move(2, { x: 280, y: 150 })).toEqual({ x: -80, y: 30, zoom: 4 });
  });
  test('a remaining finger cannot turn navigation into drawing', () => {
    const gesture = start();
    gesture.up(2, initial);
    expect(gesture.active).toBe(true);
    expect(gesture.move(1, { x: 300, y: 300 })).toBeNull();
    gesture.up(1, initial);
    expect(gesture.active).toBe(false);
    expect(gesture.down(3, { x: 100, y: 100 }, initial)).toBe(false);
  });
  test('replacing a finger starts from the current viewport without jumping', () => {
    const gesture = start();
    gesture.move(1, { x: 130, y: 150 });
    const moved = gesture.move(2, { x: 230, y: 150 })!;
    gesture.up(2, moved);
    gesture.down(3, { x: 230, y: 150 }, moved);
    expect(gesture.move(3, { x: 230, y: 150 })).toEqual(moved);
  });
  test('adding and removing a third finger rebases without a jump', () => {
    const gesture = start();
    gesture.down(3, { x: 300, y: 100 }, initial);
    gesture.up(1, initial);
    expect(gesture.move(3, { x: 300, y: 100 })).toEqual(initial);
  });
  test('zoom limits preserve the world anchor', () => {
    const gesture = start();
    const maximum = gesture.move(2, { x: 10000, y: 100 })!;
    expect(maximum.zoom).toBe(8);
    expect((5050 - maximum.x) / maximum.zoom).toBe((150 - initial.x) / initial.zoom);
    const minimum = gesture.move(2, { x: 100, y: 100 })!;
    expect(minimum.zoom).toBe(0.1);
    expect(Number.isFinite(minimum.x)).toBe(true);
  });
  test('cancellation resets contacts so the next single finger can draw', () => {
    const gesture = start(); gesture.reset();
    expect(gesture.count).toBe(0);
    expect(gesture.move(1, { x: 400, y: 400 })).toBeNull();
    expect(gesture.down(9, { x: 30, y: 30 }, initial)).toBe(false);
  });
});
