import { forwardRef, type ReactElement } from "react";
import { IconBase, type Icon, type IconProps, type IconWeight } from "@phosphor-icons/react";

/** opencode's mark on Phosphor's `IconBase`, so it takes the same `size` and
 *  `color` as the rest of the icons. One drawing for every weight: a frame
 *  with its lower half washed, the vendor's own two-tone. */
// The 240×300 mark, scaled into Phosphor's 256 box with a 24 margin.
const MARK: ReactElement = (
  <>
    <g transform="translate(44.8 24) scale(0.6933)">
      <path fillRule="evenodd" d="M0 0h240v300H0V0Zm60 60v180h120V60H60Z" />
      <path d="M60 120h120v120H60z" fillOpacity={0.35} />
    </g>
  </>
);
const WEIGHTS = new Map<IconWeight, ReactElement>(
  (["thin", "light", "regular", "bold", "fill", "duotone"] as const).map((w) => [w, MARK]),
);

export const OpencodeIcon: Icon = forwardRef<SVGSVGElement, IconProps>((props, ref) => (
  <IconBase ref={ref} {...props} weights={WEIGHTS} />
));
OpencodeIcon.displayName = "OpencodeIcon";
