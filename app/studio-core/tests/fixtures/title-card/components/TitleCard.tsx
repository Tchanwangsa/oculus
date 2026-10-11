export const props = {
  eyebrow: str(),
  title: str(),
  subtitle: str(),
  accent: token("color").default("accent"),
};

export const examples = {
  short: { eyebrow: "Week 1", title: "Vectors", subtitle: "Arrows with a length and a direction." },
};

const bars = [0.92, 0.64, 0.46, 0.33, 0.25, 0.2, 0.17, 0.155];

export default ({ eyebrow, title, subtitle, accent }, { theme }) => {
  const { color, space, radius, type } = theme;
  return (
    <Row
      align="center"
      gap={space.xl}
      style={{
        padding: [0, space.xxl],
        fill: { linear: { angle: 120, stops: [[0, color.ink], [0.55, "#262a5c"], [1, color.accentStrong]] } },
      }}
    >
      <Stack gap={space.l} style={{ flexGrow: 1, flexShrink: 1 }}>
        <Row>
          <Box style={{ padding: [space.xs, space.s + 4], radius: radius.pill, fill: "#ffffff1f", border: { width: 1.5, color: "#ffffff33" } }}>
            <Text style={{ ...type.label, color: color.onInk }}>{eyebrow}</Text>
          </Box>
        </Row>
        <Text style={{ ...type.display, color: "#ffffff" }}>{title}</Text>
        <Text style={{ ...type.body, fontSize: 36, color: "#c9cbe8", maxWidth: 820 }}>{subtitle}</Text>
      </Stack>
      <Stack
        gap={space.m}
        style={{
          width: 600,
          padding: space.xl,
          radius: radius.xl,
          fill: color.surface,
          shadow: { y: 40, blur: 120, color: "#05061a80" },
        }}
      >
        <Row justify="space-between" align="center">
          <Text style={{ ...type.h3, color: color.text }}>Loss</Text>
          <Text style={{ ...type.label, color: accent }}>epoch 1 → 8</Text>
        </Row>
        <Row align="flex-end" gap={space.s} style={{ height: 280 }}>
          {bars.map((h, i) => (
            <Box
              key={i}
              style={{
                flexGrow: 1,
                height: `${h * 100}%`,
                radius: radius.s,
                fill: { linear: { angle: 180, stops: [[0, accent], [1, color.accentSoft]] } },
              }}
            />
          ))}
        </Row>
        <Text style={{ ...type.caption, color: color.muted }}>Each step lowers the loss a little less than the last.</Text>
      </Stack>
    </Row>
  );
};
