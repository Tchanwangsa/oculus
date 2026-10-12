export const props = {};

// A number flies into a symbol's slot: the given and the formula are one
// Math, so the 3's \k key matches across the two states; r leaves with π.
export default (_p, { theme }) => (
  <Stack justify="center" align="center" style={{ fill: theme.color.bg, width: "100%", height: "100%" }}>
    <Steps at={[0.4]} dur={1.0}>
      <Math key="eq" tex="\begin{gathered} A = \pi r^2 \\ r = \k{val}{3} \end{gathered}" style={{ fontSize: 80 }} />
      <Math key="eq" tex="\begin{gathered} A = \pi \cdot \k{val}{3}^2 \\ \phantom{r = 3} \end{gathered}" style={{ fontSize: 80 }} />
    </Steps>
  </Stack>
);
