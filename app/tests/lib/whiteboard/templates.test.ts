import { describe, expect, test } from 'bun:test';
import { DEFAULT_DRAWING_STYLE } from '@/lib/canvas/style';
import { buildTemplate } from '@/lib/canvas/templates';

const centre = { x: 500, y: 300 };
const build = (kind: Parameters<typeof buildTemplate>[0]) => buildTemplate(kind, centre, DEFAULT_DRAWING_STYLE);

describe('canvas templates', () => {
  test('a matrix is a 3 by 3 grid of equal squares centred on the point', () => {
    const cells = build('matrix');
    expect(cells).toHaveLength(9);
    expect(new Set(cells.map(cell => cell.width))).toEqual(new Set([cells[0].width]));
    const left = Math.min(...cells.map(cell => cell.x)), right = Math.max(...cells.map(cell => cell.x + cell.width));
    const top = Math.min(...cells.map(cell => cell.y)), bottom = Math.max(...cells.map(cell => cell.y + cell.height));
    expect([(left + right) / 2, (top + bottom) / 2]).toEqual([centre.x, centre.y]);
  });
  test('the axes share an origin and take the x, y and z colours', () => {
    const [x, y] = build('axes2');
    expect([x.x, x.y]).toEqual([y.x, y.y]);
    expect([x.stroke, y.stroke]).toEqual(['#ef476f', '#3fb68b']);
    expect(x.points).toEqual([{ x: 0, y: 0 }, { x: x.width, y: 0 }]);
    expect(y.points?.[1].y).toBeLessThan(0);
    const [z, yy, xx] = build('axes3');
    expect([z.stroke, yy.stroke, xx.stroke]).toEqual(['#3b82f6', '#3fb68b', '#ef476f']);
    expect(new Set([z, yy, xx].map(axis => `${axis.x},${axis.y}`)).size).toBe(1);
  });
  test('every element has its own id', () => {
    const all = [...build('matrix'), ...build('axes2'), ...build('axes3')];
    expect(new Set(all.map(element => element.id)).size).toBe(all.length);
  });
});
