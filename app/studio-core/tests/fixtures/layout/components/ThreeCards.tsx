export const props = {
  heading: str(),
  cards: list(str()),
  points: list(str()),
  notes: list(str()),
};

export const examples = {
  two: { heading: "Two cards", cards: ["A", "B"], points: ["One point"], notes: ["a", "b"] },
};

const Card = ({ title, note, children }, { theme }) => (
  <Stack
    style={{
      flexGrow: 1,
      flexShrink: 1,
      width: 0,
      radius: theme.radius.l,
      fill: theme.color.surface,
      border: { width: 2, color: theme.color.border },
      shadow: { y: 12, blur: 40, color: theme.color.shadow },
      justifyContent: "space-between",
      clip: true,
    }}
  >
    {children}
    <Stack gap={theme.space.xs} style={{ padding: [theme.space.m, theme.space.l, theme.space.l, theme.space.l] }}>
      <Text style={{ ...theme.type.h3, color: theme.color.text }}>{title}</Text>
      <Text style={{ ...theme.type.body, fontSize: 28, color: theme.color.muted }}>{note}</Text>
    </Stack>
  </Stack>
);

export default ({ heading, cards, points, notes }, { theme }) => {
  const { color, space, radius, type } = theme;
  return (
    <Stack gap={space.l} style={{ padding: [space.xl, space.xxl], fill: color.bg }}>
      <Row justify="space-between" align="flex-end">
        <Text style={{ ...type.h1, color: color.text }}>{heading}</Text>
        <Box style={{ padding: [space.xs, space.s], radius: radius.pill, fill: color.accentSoft }}>
          <Text style={{ ...type.label, color: color.accentStrong }}>3 options</Text>
        </Box>
      </Row>
      <Row gap={space.l} align="stretch" style={{ flexGrow: 1 }}>
        <Card title={cards[0]} note={notes[0]}>
          <Image src="hills.png" style={{ flexGrow: 1, minHeight: 300, objectFit: "cover" }} />
        </Card>
        <Card title={cards[1]} note={notes[1]}>
          <Stack gap={space.s} style={{ flexGrow: 1, padding: space.l, fill: color.surfaceAlt, maxHeight: 520 }}>
            {points.map((p, i) => (
              <Row key={i} gap={space.s} align="center">
                <Box style={{ width: 40, height: 40, radius: 20, fill: color.accent, alignItems: "center", justifyContent: "center" }}>
                  <Text style={{ ...type.label, color: color.onAccent }}>{i + 1}</Text>
                </Box>
                <Text style={{ ...type.body, fontSize: 28, color: color.text, flexShrink: 1 }}>{p}</Text>
              </Row>
            ))}
          </Stack>
        </Card>
        <Card title={cards[2]} note={notes[2]}>
          <Box style={{ flexGrow: 1, fill: color.ink, alignItems: "center", justifyContent: "center", clip: true }}>
            <Box style={{ position: "absolute", left: -80, top: 120, width: 900, height: 90, rotate: -18, fill: { linear: { angle: 90, stops: [[0, color.accent], [1, "#c86dd7"]] } } }} />
            <Box style={{ width: 150, height: 150, radius: 75, fill: "#ffffff", opacity: 0.92, shadow: { y: 16, blur: 48, color: "#00000066" } }} />
            <Box style={{ position: "absolute", right: -60, bottom: -60, width: 200, height: 200, radius: 100, fill: color.warning, blur: 18 }} />
          </Box>
        </Card>
      </Row>
    </Stack>
  );
};
