export const props = { title: str(), point: str() };
export const examples = { basic: { title: "Gradient descent", point: "Step downhill" } };

// The card rises in as the narration says "gradient"; the caption fades in
// once "small step" has been said.
export default ({ title, point }, { theme }) => {
  const { color, space, radius } = theme;
  return (
    <Stack gap={space.m} justify="center" style={{ padding: [0, 48] }}>
      <Text style={{ fontFamily: "Manrope", fontSize: 44, fontWeight: 700, letterSpacing: -1, color: color.text }}>{title}</Text>
      <Enter preset="rise" at={word("gradient")} dur={0.4}>
        <Row align="center" style={{ height: 64, padding: [0, space.m], radius: radius.m, fill: color.accent }}>
          <Text style={{ fontFamily: "Inter", fontSize: 24, fontWeight: 600, color: color.onAccent }}>{point}</Text>
        </Row>
      </Enter>
      <Enter preset="fade" at={wordEnd("small step")} dur={0.3}>
        <Text style={{ fontFamily: "Inter", fontSize: 18, color: color.muted }}>One small step at a time</Text>
      </Enter>
    </Stack>
  );
};
