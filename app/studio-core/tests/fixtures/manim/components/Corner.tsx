export const props = {};

// Shrink to the corner: the same Math key re-laid out small at the top left
// glides and scales its glyphs, and a new formula arrives in the middle.
export default (_p, { theme }) => (
  <Box style={{ fill: theme.color.bg, width: "100%", height: "100%" }}>
    <Steps at={[0.5]} dur={1.2}>
      <Stack justify="center" align="center" style={{ width: "100%", height: "100%" }}>
        <Math key="eq" tex="a^2 - b^2 = (a - b)(a + b)" style={{ fontSize: 80 }} />
      </Stack>
      <Box style={{ width: "100%", height: "100%" }}>
        <Math key="eq" tex="a^2 - b^2 = (a - b)(a + b)" style={{ fontSize: 32, position: "absolute", left: 40, top: 32 }} />
        <Stack justify="center" align="center" style={{ width: "100%", height: "100%" }}>
          <Math key="next" tex="2^4 - 1 = (2^2 - 1)(2^2 + 1)" style={{ fontSize: 80 }} />
        </Stack>
      </Box>
    </Steps>
  </Box>
);
