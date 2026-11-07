/**
 * `@react-three/fiber` for widgets: r3f 9.8 as published, except `Canvas`,
 * which is the kit's releasing canvas (`3d/canvas/Canvas.tsx`): r3f's own
 * would keep its WebGL context off-screen, and WebKit loses contexts past
 * about sixteen. The explicit export wins over the star one.
 */
export * from "@react-three/fiber";
export { Canvas } from "./3d/canvas/Canvas";
