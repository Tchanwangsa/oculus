export const props = {};

const cells = ["Data", "Model", "Loss", "Gradient", "Update", "Repeat"];

export default ({}, { t, theme }) => {
  const { color, space, radius, type } = theme;
  const p = interpolate(t, [0, 2], [0, 1], { easing: easeInOutCubic });
  return (
    <Camera x={lerp(960, 1290, p)} y={lerp(540, 690, p)} zoom={lerp(1, 1.8, p)} rotate={lerp(0, -4, p)}>
      <Stack gap={space.l} justify="center" style={{ flexGrow: 1, padding: [0, 200], fill: color.bg }}>
        <Text style={{ ...type.h1, color: color.text }}>One training step</Text>
        <Row wrap gap={space.m}>
          {cells.map((c, i) => (
            <Box key={c} style={{ width: 480, height: 200, padding: space.l, radius: radius.l, fill: i === 4 ? color.accent : color.surface, shadow: { y: 16, blur: 48, color: color.shadow } }}>
              <Text style={{ ...type.h3, color: i === 4 ? color.onAccent : color.text }}>{`${i + 1}. ${c}`}</Text>
            </Box>
          ))}
        </Row>
      </Stack>
    </Camera>
  );
};
