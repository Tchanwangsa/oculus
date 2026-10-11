export const props = {
  eyebrow: str().optional(),
  title: str(),
  subtitle: str().optional(),
  accent: token("color").default("accent"),
};

export const examples = {
  short: { title: "Vectors" },
  long: {
    eyebrow: "Lecture 4",
    title: "Gradient descent",
    subtitle: "Step downhill until the slope flattens out.",
  },
};

export default ({ eyebrow, title, subtitle, accent }, { theme }) => {
  const { color, space, radius, type } = theme;
  return (
    <Stack justify="center" gap={space.m} style={{ flexGrow: 1, padding: space.xxl, fill: color.ink }}>
      {eyebrow ? (
        <Row>
          <Box style={{ padding: [space.xs, space.s], radius: radius.pill, fill: accent }}>
            <Text style={{ ...type.label, color: color.onAccent }}>{eyebrow}</Text>
          </Box>
        </Row>
      ) : null}
      <Text style={{ ...type.display, color: color.onInk }}>{title}</Text>
      {subtitle ? <Text style={{ ...type.body, color: color.faint, maxWidth: 960 }}>{subtitle}</Text> : null}
    </Stack>
  );
};
