import { expect, test } from "bun:test";
import { proseBands } from "@/lib/markdown/mathSelection/proseGeometry";

const piece = (left: number, right: number, top: number, bottom: number, baseline: number) => ({box:{left,right,top,bottom},baseline});

test("small inline maths takes the text’s full selection height", () => {
  const pieces=[piece(0,40,0,18,14),piece(40,50,6,13,14),piece(50,100,0,18,14)];
  expect(proseBands(pieces)).toEqual([{left:0,right:100,top:0,bottom:18}]);
  expect(pieces[1].box.top).toBe(6);
});

test("a tall fraction gives the adjacent prose the same full highlight height", () => {
  expect(proseBands([piece(0,40,8,26,22),piece(40,60,0,38,22),piece(60,100,8,26,22)]))
    .toEqual([{left:0,right:100,top:0,bottom:38}]);
});

test("wrapped lines stay separate even when tall ink overlaps vertically", () => {
  expect(proseBands([piece(0,60,0,32,14),piece(0,80,20,38,34)]))
    .toEqual([{left:0,right:60,top:0,bottom:32},{left:0,right:80,top:20,bottom:38}]);
});

test("fractional baselines from different faces join the same line", () => {
  expect(proseBands([piece(0,20,0,18,14),piece(20,40,-1,20,14.5)]))
    .toEqual([{left:0,right:40,top:-1,bottom:20}]);
});
