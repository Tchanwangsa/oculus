export const props = {};

// TransformMatchingTex: both formulas share the Math key, so the 2, the
// minus and the 1 glide into the factored form and the rest fades.
export default (_p, { t, theme }) => (
  <Stack justify="center" align="center" gap={24} style={{ fill: theme.color.bg, width: "100%", height: "100%" }}>
    <Text style={{ ...theme.type.label, color: theme.color.muted }}>Factor</Text>
    <Morph p={clamp(t, 0, 1)}>
      <Math key="eq" tex="2^4 - 1" style={{ fontSize: 64 }} />
      <Math key="eq" tex="(2^2 - 1)(2^2 + 1)" style={{ fontSize: 64 }} />
    </Morph>
  </Stack>
);
