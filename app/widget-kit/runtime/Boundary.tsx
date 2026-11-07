import { Component, type ReactNode } from "react";
import { describe, post } from "./post";

/** Catches a render throw anywhere in the widget, reports it and draws
 *  nothing: the parent shows the error under its title. */
export class Boundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: unknown) {
    post({ kind: "error", phase: "render", message: describe(error) });
  }

  render() {
    return this.state.failed ? null : this.props.children;
  }
}
