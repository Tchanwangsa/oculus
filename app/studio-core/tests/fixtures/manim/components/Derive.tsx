export const props = {};

// A derivation rewritten in place over four states: shared terms glide,
// the rest fade in beside their neighbours, staggered in reading order.
export default (_p, { theme }) => (
  <Stack justify="center" align="center" style={{ fill: theme.color.bg, width: "100%", height: "100%" }}>
    <Steps at={[0.5, 1.75, 3.0]} dur={1.0} lag={0.1}>
      <Math key="eq" tex="x^2 - 5x + 6 = 0" style={{ fontSize: 72 }} />
      <Math key="eq" tex="(x - 2)(x - 3) = 0" style={{ fontSize: 72 }} />
      <Math key="eq" tex="x - 2 = 0 \quad \text{or} \quad x - 3 = 0" style={{ fontSize: 72 }} />
      <Math key="eq" tex="x = 2 \quad \text{or} \quad x = 3" style={{ fontSize: 72 }} />
    </Steps>
  </Stack>
);
