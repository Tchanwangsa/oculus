export const props = {
  value: str(),
  unit: str().optional(),
  label: str(),
  accent: token("color").default("accent"),
};

export const examples = {
  percent: { value: "92%", label: "of the cohort finished the set" },
  count: { value: "1,024", unit: "frames", label: "rendered for one lecture" },
};

export default ({ value, unit, label, accent }, { theme }) => {
  const { color, space, type } = theme;
  return (
    <Stack justify="center" align="center" gap={space.s} style={{ flexGrow: 1, padding: space.xl, fill: color.surface }}>
      <Row align="flex-end" gap={space.xs}>
        <Text style={{ ...type.display, fontSize: 176, color: accent, fontVariantNumeric: "tabular-nums" }}>{value}</Text>
        {unit ? <Text style={{ ...type.h2, color: color.muted }}>{unit}</Text> : null}
      </Row>
      <Text style={{ ...type.body, color: color.muted }}>{label}</Text>
    </Stack>
  );
};
