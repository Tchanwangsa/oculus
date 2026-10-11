export const props = {
  heading: str(),
  leftTitle: str().default("Left"),
  left: list(str()),
  rightTitle: str().default("Right"),
  right: list(str()),
  accent: token("color").default("accent"),
};

export const examples = {
  slope: {
    heading: "Two views of one slope",
    leftTitle: "Height",
    left: ["The loss falls", "Steps shrink"],
    rightTitle: "Slope",
    right: ["The slope flattens", "Stop near zero"],
  },
  short: { heading: "Compare", left: ["A"], right: ["B"] },
};

const Column = ({ title, items, accent }, { theme }) => {
  const { color, space, radius, type } = theme;
  return (
    <Stack
      gap={space.s}
      style={{
        flexGrow: 1,
        flexShrink: 1,
        width: 0,
        padding: space.l,
        radius: radius.l,
        fill: color.surface,
        border: { width: 2, color: color.border },
      }}
    >
      <Box style={{ width: 72, height: 6, radius: radius.pill, fill: accent }} />
      <Text style={{ ...type.h3, color: color.text }}>{title}</Text>
      {items.map((item, i) => (
        <Text key={`${title}-${i}`} style={{ ...type.body, color: color.muted }}>{item}</Text>
      ))}
    </Stack>
  );
};

export default ({ heading, leftTitle, left, rightTitle, right, accent }, { theme }) => {
  const { color, space, type } = theme;
  return (
    <Stack gap={space.l} style={{ flexGrow: 1, padding: space.xl, fill: color.bg }}>
      <Text style={{ ...type.h1, color: color.text }}>{heading}</Text>
      <Row gap={space.l} align="stretch" style={{ flexGrow: 1 }}>
        <Column title={leftTitle} items={left} accent={accent} />
        <Column title={rightTitle} items={right} accent={accent} />
      </Row>
    </Stack>
  );
};
