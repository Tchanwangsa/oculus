import { expect, test } from "bun:test";
import { lineBands } from "@/lib/maths/geometry/lineBands";

const piece = (left: number, right: number, top: number, bottom: number, baseline: number) => ({box:{left,right,top,bottom},baseline});

test("small inline maths takes the text’s full selection height", () => {
  const pieces=[piece(0,40,0,18,14),piece(40,50,6,13,14),piece(50,100,0,18,14)];
  expect(lineBands(pieces)).toEqual([{left:0,right:100,top:0,bottom:18}]);
  expect(pieces[1].box.top).toBe(6);
});

test("a tall fraction gives the adjacent prose the same full highlight height", () => {
  expect(lineBands([piece(0,40,8,26,22),piece(40,60,0,38,22),piece(60,100,8,26,22)]))
    .toEqual([{left:0,right:100,top:0,bottom:38}]);
});

test("wrapped lines stay separate even when tall ink overlaps vertically", () => {
  expect(lineBands([piece(0,60,0,32,14),piece(0,80,20,38,34)]))
    .toEqual([{left:0,right:60,top:0,bottom:32},{left:0,right:80,top:20,bottom:38}]);
});

test("fractional baselines from different faces join the same line", () => {
  expect(lineBands([piece(0,20,0,18,14),piece(20,40,-1,20,14.5)]))
    .toEqual([{left:0,right:40,top:-1,bottom:20}]);
});

test("WebKit text metrics and matrix struts still join after pixel rounding", () => {
  expect(lineBands([piece(0,40,16.51,43.47,36.51),piece(40,90,0,64,38.586),piece(90,150,16.51,43.47,36.51)]))
    .toEqual([{left:0,right:150,top:0,bottom:64}]);
});

test("disjoint selected runs share height without painting the gap between them", () => {
  expect(lineBands([piece(0,40,8,26,22),piece(70,90,0,38,22)]))
    .toEqual([{left:0,right:40,top:0,bottom:38},{left:70,right:90,top:0,bottom:38}]);
});
