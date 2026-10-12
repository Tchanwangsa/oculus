export const props = {};

// Overlays on keys, drawn after the morph: the brace and its label follow
// the factored form as it rewrites, the surround draws on round the 1, the
// minus pulses, and an arrow from a point lands on the 4's edge.
export default (_p, { theme }) => (
  <Stack justify="center" align="center" style={{ fill: theme.color.bg, width: "100%", height: "100%" }}>
    <Steps at={[2.0]} dur={1.0}>
      <Math key="eq" tex="2^4 - 1" style={{ fontSize: 96 }} />
      <Math key="eq" tex="(2^2 - 1)(2^2 + 1)" style={{ fontSize: 96 }} />
    </Steps>
    <Brace target="eq" label="fifteen" at={0.2} dur={0.6} />
    <Surround target="eq/1" at={0.6} dur={0.6} />
    <Underline target="eq/2" at={0.6} />
    <Indicate target="eq/−" at={0.9} dur={1.0} />
    <Arrow from={[260, 140]} to="eq/4" bend={-30} at={0.3} dur={0.7} />
  </Stack>
);
