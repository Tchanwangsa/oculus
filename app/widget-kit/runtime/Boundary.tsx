import { Component, type ReactNode } from "react";
import { describe, post } from "./post";

/** Catches a render throw anywhere in the widget and draws nothing. A
 *  committed widget reports it (the parent shows the error under its
 *  title); a preview passes `onError` instead, which puts the last good
 *  preview back. */
export class Boundary extends Component<{ children: ReactNode; onError?: (error: unknown) => void }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: unknown) {
    if (this.props.onError) this.props.onError(error);
    else post({ kind: "error", phase: "render", message: describe(error) });
  }

  render() {
    return this.state.failed ? null : this.props.children;
  }
}
