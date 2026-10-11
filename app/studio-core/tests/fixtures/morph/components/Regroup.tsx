export const props = {};

// Card C moves from the first row to the second and the cards grow, so the
// footer below the Morph glides down with it.
const Card = ({ label, tall }, { theme }) => (
  <Box style={{ width: 120, height: tall ? 84 : 60, padding: 12, radius: theme.radius.m, fill: label === "C" ? theme.color.accent : theme.color.surface, shadow: { y: 6, blur: 18, color: theme.color.shadow } }}>
    <Text style={{ fontFamily: "Inter", fontSize: 18, fontWeight: 600, color: label === "C" ? "#ffffff" : theme.color.text }}>{label}</Text>
  </Box>
);

export default (_p, { t, theme }) => (
  <Stack gap={16} style={{ padding: 32 }}>
    <Text style={{ fontFamily: "Manrope", fontSize: 28, fontWeight: 700, color: theme.color.text }}>Regroup</Text>
    <Morph p={easeInOutCubic(clamp(t, 0, 1))}>
      <Stack gap={12}>
        <Row key="r1" gap={12}><Card key="a" label="A" /><Card key="b" label="B" /><Card key="c" label="C" /></Row>
        <Row key="r2" gap={12}><Card key="d" label="D" /></Row>
      </Stack>
      <Stack gap={12}>
        <Row key="r1" gap={12}><Card key="a" label="A" tall /><Card key="b" label="B" tall /></Row>
        <Row key="r2" gap={12}><Card key="c" label="C" tall /><Card key="d" label="D" tall /></Row>
      </Stack>
    </Morph>
    <Box style={{ padding: [10, 16], radius: theme.radius.pill, fill: theme.color.accentSoft, alignSelf: "flex-start" }}>
      <Text style={{ fontFamily: "Inter", fontSize: 14, fontWeight: 600, color: theme.color.accentStrong }}>The footer follows the layout</Text>
    </Box>
  </Stack>
);
