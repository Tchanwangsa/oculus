export const props = {
  heading: str().optional(),
  steps: list(str()),
  accent: token("color").default("accent"),
};

export const examples = {
  three: {
    heading: "Gradient descent",
    steps: ["Pick a starting point", "Measure the slope", "Step against it"],
  },
  two: { steps: ["Read the loss", "Take a step"] },
};

export default ({ heading, steps, accent }, { theme }) => {
  const { color, space, radius, type } = theme;
  return (
    <Stack gap={space.l} style={{ flexGrow: 1, padding: space.xl, fill: color.bg }}>
      {heading ? <Text style={{ ...type.h1, color: color.text }}>{heading}</Text> : null}
      <Stack gap={space.m}>
        {steps.map((step, i) => (
          <Row key={i} gap={space.m} align="center">
            <Box
              style={{
                width: 52,
                height: 52,
                radius: radius.pill,
                fill: accent,
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Text style={{ ...type.label, color: color.onAccent }}>{i + 1}</Text>
            </Box>
            <Text style={{ ...type.h3, color: color.text }}>{step}</Text>
          </Row>
        ))}
      </Stack>
    </Stack>
  );
};
