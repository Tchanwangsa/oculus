export const props = { title: str(), variant: oneOf(["a", "b"]) };
export const examples = { light: { title: "Push", variant: "a" } };

// Two looks with the same keyed parts, so a magic move has pairs to blend.
export default ({ title, variant }, { theme }) => {
  const { color, radius } = theme;
  const dark = variant === "b";
  const card = (
    <Stack key="card" gap={10} style={{ width: dark ? 260 : 220, padding: 20, radius: dark ? radius.s : radius.l, fill: dark ? color.warning : color.accent, shadow: { y: 10, blur: 30, color: color.shadow } }}>
      <Text key="label" style={{ fontFamily: "Inter", fontSize: 14, fontWeight: 600, color: "#ffffff" }}>Step size</Text>
      <Row key="bars" gap={8} align="flex-end" style={{ height: dark ? 120 : 90 }}>
        {[0.9, 0.6, 0.45, 0.3, 0.25].map((h, i) => (
          <Box key={`bar${i}`} style={{ flexGrow: 1, height: `${(dark ? 1.1 - h : h) * 100}%`, radius: 4, fill: "#ffffffcc" }} />
        ))}
      </Row>
    </Stack>
  );
  const heading = (
    <Stack key="heading" gap={8} style={{ flexShrink: 1 }}>
      <Text key="title" style={{ fontFamily: "Manrope", fontSize: dark ? 52 : 44, fontWeight: 700, letterSpacing: -1, color: dark ? color.onInk : color.text }}>{title}</Text>
      <Text key="sub" style={{ fontFamily: "Inter", fontSize: 16, color: dark ? color.faint : color.muted }}>A transition, frame by frame</Text>
    </Stack>
  );
  return (
    <Row gap={40} align="center" style={{ padding: [0, 48], fill: dark ? color.ink : color.bg }}>
      {dark ? [card, heading] : [heading, card]}
    </Row>
  );
};
