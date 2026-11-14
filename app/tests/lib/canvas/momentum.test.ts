import { describe, expect, test } from 'bun:test';
import { DECELERATION, MAX_SPEED, STOP_SPEED, VelocityTracker, eventTime, glide, gliding } from '../../../src/lib/canvas/momentum';

/** A pan sampled every 16 ms from `from` ms, moving `step` pixels a sample, `count` samples long. */
const pan = (count: number, step: { x: number; y: number }, zoom = 1, from = 0) => {
  const tracker = new VelocityTracker();
  tracker.start(from, { x: 0, y: 0, zoom });
  for (let index = 1; index <= count; index++) tracker.add(from + index * 16, { x: step.x * index, y: step.y * index, zoom });
  return { tracker, end: from + count * 16 };
};

describe('release velocity', () => {
  test('a steady flick releases at its speed', () => {
    const { tracker, end } = pan(10, { x: 0, y: -32 });
    const velocity = tracker.release(end + 4)!;
    expect(velocity.x).toBe(0);
    expect(velocity.y).toBeCloseTo(-2);
  });
  test('only the last 100 ms count, so a pan that speeds up releases at its final speed', () => {
    const tracker = new VelocityTracker();
    tracker.start(0, { x: 0, y: 0, zoom: 1 });
    tracker.add(400, { x: 40, y: 0, zoom: 1 });
    for (let time = 416; time <= 496; time += 16) tracker.add(time, { x: 40 + (time - 400) * 3, y: 0, zoom: 1 });
    expect(tracker.release(500)!.x).toBeCloseTo(3);
  });
  test('a finger held still before lifting does not fling', () => {
    const { tracker, end } = pan(10, { x: 30, y: 0 });
    tracker.add(end + 60, { x: 300, y: 0, zoom: 1 });
    expect(tracker.release(end + 60)).toBeNull();
  });
  test('a pinch does not fling, but two panning fingers drifting or wobbling between their events do', () => {
    const tracker = new VelocityTracker();
    tracker.start(0, { x: 0, y: 0, zoom: 1 });
    for (let index = 1; index <= 6; index++) tracker.add(index * 16, { x: index * 20, y: 0, zoom: 1 + index * 0.05 });
    expect(tracker.release(100)).toBeNull();
    tracker.start(0, { x: 0, y: 0, zoom: 1 });
    for (let index = 1; index <= 6; index++) tracker.add(index * 16, { x: index * 20, y: 0, zoom: 1 + index * 0.002 });
    expect(tracker.release(100)!.x).toBeCloseTo(1.25);
    tracker.start(0, { x: 0, y: 0, zoom: 1 });
    for (let index = 1; index <= 6; index++) tracker.add(index * 16, { x: index * 20, y: 0, zoom: index % 2 ? 1.08 : 1 });
    expect(tracker.release(100)!.x).toBeCloseTo(1.25);
  });
  test('a slow release does not fling and a violent one is capped', () => {
    expect(pan(6, { x: 2, y: 2 }).tracker.release(100)).toBeNull();
    const fast = pan(6, { x: 300, y: 400 }).tracker.release(100)!;
    expect(Math.hypot(fast.x, fast.y)).toBeCloseTo(MAX_SPEED);
    expect(fast.x / fast.y).toBeCloseTo(0.75);
  });
  test('a tap without movement does not fling', () => {
    const tracker = new VelocityTracker();
    tracker.start(0, { x: 5, y: 5, zoom: 1 });
    tracker.add(30, { x: 5, y: 5, zoom: 1 });
    expect(tracker.release(40)).toBeNull();
  });
});

describe('false flicks', () => {
  test('moves handled in a burst after a slow frame do not read as a violent flick', () => {
    const tracker = new VelocityTracker();
    tracker.start(0, { x: 0, y: 0, zoom: 1 });
    tracker.add(200, { x: 0, y: 30, zoom: 1 });
    tracker.add(200.4, { x: 0, y: 34, zoom: 1 });
    expect(tracker.release(201)).toBeNull();
  });
  test('a tap or a resting hand that jitters as it lifts does not fling', () => {
    const tracker = new VelocityTracker();
    tracker.start(0, { x: 0, y: 0, zoom: 1 });
    for (let index = 1; index <= 4; index++) tracker.add(index * 16, { x: 0, y: index * 4, zoom: 1 });
    expect(tracker.release(70)).toBeNull();
  });
  test('two fingers that zoomed over the gesture do not glide, even if the last moment was a steady pan', () => {
    const tracker = new VelocityTracker();
    tracker.start(0, { x: 400, y: 400, zoom: 2 });
    for (let index = 1; index <= 10; index++) tracker.add(index * 16, { x: 400, y: 400 - index * 30, zoom: index < 5 ? 2 + index * 0.1 : 2.4 });
    expect(tracker.release(165)).toBeNull();
    expect(tracker.lastRelease?.verdict).toBe('pinch');
  });
  test('a sample is timed by its event, unless the stamp is missing or on another clock', () => {
    expect(eventTime({ timeStamp: 950 }, 1000)).toBe(950);
    expect(eventTime({}, 1000)).toBe(1000);
    expect(eventTime({ timeStamp: 0 }, 1000)).toBe(1000);
    expect(eventTime({ timeStamp: 1_700_000_000_000 }, 1000)).toBe(1000);
  });
});

describe('glide', () => {
  test('decelerates like UIScrollView and covers the same path at any frame rate', () => {
    const start = { x: 2, y: -1 };
    const once = glide(start, 96);
    let velocity = start, dx = 0, dy = 0;
    for (let frame = 0; frame < 12; frame++) {
      const step = glide(velocity, 8);
      dx += step.dx; dy += step.dy; velocity = step.velocity;
    }
    expect(once.velocity.x).toBeCloseTo(2 * DECELERATION ** 96);
    expect(velocity.x).toBeCloseTo(once.velocity.x);
    expect(dx).toBeCloseTo(once.dx);
    expect(dy).toBeCloseTo(once.dy);
    expect(once.dx).toBeGreaterThan(0);
    expect(once.dx).toBeLessThan(2 * 96);
  });
  test('travels v / -ln(r) in total and stops below the stop speed', () => {
    const total = glide({ x: 1, y: 0 }, 100000);
    expect(total.dx).toBeCloseTo(-1 / Math.log(DECELERATION));
    expect(gliding({ x: 1, y: 0 })).toBe(true);
    expect(gliding({ x: STOP_SPEED / 2, y: 0 })).toBe(false);
  });
  test('no time, no movement', () => {
    const still = glide({ x: 3, y: 4 }, 0);
    expect(Math.abs(still.dx) + Math.abs(still.dy)).toBe(0);
    expect(still.velocity).toEqual({ x: 3, y: 4 });
  });
});
