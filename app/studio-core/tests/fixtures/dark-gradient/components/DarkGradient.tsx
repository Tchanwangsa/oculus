export const props = {};
export const examples = { only: {} };

// A full-frame left-to-right gradient across the dark theme's range: the
// worst case for 8-bit banding.
export default () => (
  <Box style={{ flexGrow: 1, fill: { linear: { angle: 90, stops: [[0, "#0e0f13"], [1, "#1c1e26"]] } } }} />
);
