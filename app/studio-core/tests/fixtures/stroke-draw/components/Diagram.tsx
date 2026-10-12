export const props = {};

export default ({}, { t, theme }) => {
  const { color, space, type } = theme;
  const draw = interpolate(t, [0, 1.2], [0, 1], { easing: easeInOutCubic });
  return (
    <Box style={{ fill: color.bg }}>
      <Stack gap={space.l} style={{ padding: [space.xxl, 160] }}>
        <Text style={{ ...type.h2, color: color.text }}>Loss along the path</Text>
        <Path
          d="M 0 300 C 150 0 300 0 450 200 S 750 420 900 60"
          viewBox={[0, -20, 900, 340]}
          stroke={{ width: 8, color: color.accent, cap: "round" }}
          trim={[0, draw]}
          style={{ width: 900 }}
        />
        <Path
          d="M 0 0 L 900 0"
          stroke={{ width: 3, color: color.faint, dash: [14, 10] }}
          trim={[0, draw]}
          style={{ width: 900 }}
        />
        <Row gap={space.l} align="center">
          <Path
            d="M 60 0 A 60 60 0 1 1 60 120 A 60 60 0 1 1 60 0 Z"
            fill={color.accentSoft}
            stroke={{ width: 5, color: color.accentStrong }}
            trim={[0, draw]}
            style={{ width: 120 }}
          />
          <Path
            d="M 0 0 L 120 0 L 120 120 L 0 120 Z M 30 30 L 30 90 L 90 90 L 90 30 Z"
            fill={{ linear: { angle: 135, stops: [[0, color.accent], [1, color.positive]] } }}
            style={{ width: 120 }}
          />
        </Row>
      </Stack>
      <Arrow
        points={[[1300, 260], [1560, 520], [1280, 760]]}
        curve={1}
        progress={interpolate(t, [0.3, 1.5], [0, 1])}
        stroke={{ width: 7, color: color.text }}
      />
      <Arrow points={[[1150, 900], [1700, 900]]} progress={draw} stroke={{ width: 4, color: color.negative, dash: [16, 12], cap: "butt" }} />
    </Box>
  );
};
