export const props = {};

// Each \k group is written on its own cue, as its words are said; the =
// is in no group, so it writes with `one`, the nearest listed group before it.
export default (_p, { theme }) => (
  <Stack justify="center" align="center" style={{ fill: theme.color.bg, width: "100%", height: "100%" }}>
    <Write at={{ lhs: 0.2, minus: 1.0, one: 1.4, rhs: 1.9 }} dur={0.8}>
      <Math key="eq" tex="\k{lhs}{2^4} \k{minus}{-} \k{one}{1} = \k{rhs}{\frac{a + b}{\sqrt{x}}}" style={{ fontSize: 96 }} />
    </Write>
  </Stack>
);
