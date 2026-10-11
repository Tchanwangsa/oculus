export const props = {
  title: str(),
  count: num().default(3),
  accent: token("color").default("accent"),
};

export const examples = {
  short: { title: "Short" },
  long: { title: "A longer title that wraps", count: 9 },
};

export default ({ title, count, accent }, { theme }) => (
  <Stack gap={theme.space.s} style={{ padding: theme.space.l, fill: theme.color.surface, radius: theme.radius.m, width: 520 }}>
    <Text style={{ ...theme.type.h2, color: theme.color.text }}>{title}</Text>
    <Row gap={theme.space.xs}>
      {Array.from({ length: count }, (_, i) => (
        <Box key={i} style={{ width: 20, height: 20, radius: 10, fill: accent }} />
      ))}
    </Row>
  </Stack>
);
