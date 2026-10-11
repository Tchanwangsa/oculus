export const props = {};

const Stat = ({ label, value }, { theme }) => {
  const { color, space, radius, type } = theme;
  return (
    <Stack gap={space.s} style={{ flexGrow: 1, padding: space.xl, radius: radius.l, fill: color.surface, shadow: { y: 20, blur: 60, color: color.shadow } }}>
      <Text style={{ ...type.label, color: color.muted }}>{label}</Text>
      <Text style={{ ...type.display, fontFamily: "Inter", fontVariantNumeric: "tabular-nums", color: color.text }}>{value}</Text>
    </Stack>
  );
};

export default ({}, { t, theme }) => {
  const { space } = theme;
  return (
    <Row align="center" gap={space.l} style={{ padding: [0, space.xxl] }}>
      <Stat label="Steps" value={counter(t, 0, 12840, { dur: 2 })} />
      <Stat label="Accuracy" value={`${counter(t, 50, 98.6, { at: 0.2, dur: 2, decimals: 1 })}%`} />
      <Stat label="Loss" value={counter(t, 2.5, 0.031, { dur: 2, decimals: 3 })} />
    </Row>
  );
};
