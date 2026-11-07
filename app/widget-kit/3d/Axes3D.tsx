import type { Colour } from "../colors";
import { Arrow3D } from "./Arrow3D";
import type { Vec3 } from "./geometry/vec";
import { Label3D } from "./Label3D";
import { useThickness } from "./scene";

/** How far past its tip an axis's name sits, at the default view. */
const LABEL_GAP = 0.18;

const AXES = [
  { name: "x", along: [1, 0, 0] },
  { name: "y", along: [0, 1, 0] },
  { name: "z", along: [0, 0, 1] },
] as const;

/**
 * The x, y and z axes through the origin, each from `-size` to `size` (one
 * half-length, or one per axis) with an arrow at the positive end, and their
 * names in maths at the tips unless `labels` is false.
 */
export function Axes3D({
  size = 1.5,
  labels = true,
  color = "muted",
}: {
  size?: number | Vec3;
  labels?: boolean;
  color?: Colour;
}) {
  const half: Vec3 = typeof size === "number" ? [size, size, size] : size;
  const gap = useThickness(LABEL_GAP);
  return (
    <group>
      {AXES.map(({ name, along }, i) => {
        const tip = along.map((a) => a * half[i]) as Vec3;
        const tail = along.map((a) => -a * half[i]) as Vec3;
        const label = along.map((a) => a * (half[i] + gap)) as Vec3;
        return (
          <group key={name}>
            <Arrow3D tail={tail} tip={tip} color={color} weight={0.7} />
            {labels && <Label3D at={label} tex={name} color={color} />}
          </group>
        );
      })}
    </group>
  );
}
