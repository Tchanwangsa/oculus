import type { ReactNode } from "react";
import { cx } from "./cx";

/** A small pill button, as in the app's study widgets. */
export function Pill({
  children,
  onClick,
  tone = "outline",
  pressed,
  disabled,
}: {
  children?: ReactNode;
  onClick?: () => void;
  tone?: "outline" | "primary" | "ghost";
  pressed?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      className={cx("o-pill", `o-pill-${tone}`)}
      onClick={onClick}
      aria-pressed={pressed}
      disabled={disabled}
    >
      {children}
    </button>
  );
}
