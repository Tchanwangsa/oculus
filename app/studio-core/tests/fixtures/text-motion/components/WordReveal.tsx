export const props = {
  line: str(),
  spring: token("motion.spring").default("bouncy"),
};

export const examples = {
  short: { line: "Hello there" },
};

export default ({ line, spring: preset }, { t, theme }) => {
  const { color, space, type, motion } = theme;
  const words = line.split(" ");
  const card = spring(t, motion.spring.gentle);
  const underline = interpolate(t, [0.9, 1.6], [0, 1], { easing: cubicBezier(...motion.easing.standard) });
  return (
    <Stack align="center" justify="center" style={{ fill: { linear: { angle: 160, stops: [[0, color.bg], [1, color.accentSoft]] } } }}>
      <Stack
        gap={space.l}
        style={{
          width: 1400,
          padding: space.xxl,
          radius: theme.radius.xl,
          fill: color.surface,
          shadow: { y: 30, blur: 90, color: color.shadow },
          scale: 0.92 + 0.08 * card,
          opacity: clamp(card * 1.6, 0, 1),
        }}
      >
        <Row wrap gap={space.s}>
          {words.map((w, i) => {
            const p = spring(t - 0.25 - i * motion.duration.stagger, preset);
            return (
              <Text key={i} style={{ ...type.h1, color: color.text, opacity: clamp(p, 0, 1), translateY: (1 - p) * 60 }}>
                {w}
              </Text>
            );
          })}
        </Row>
        <Box style={{ height: 8, width: `${underline * 100}%`, radius: 4, fill: mix(color.accentSoft, color.accent, underline) }} />
      </Stack>
    </Stack>
  );
};
