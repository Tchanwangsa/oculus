export const props = {};

// One formula of each construct Math draws, in two columns.
const LEFT = [
  "\\frac{1}{1 + \\frac{1}{x}}",
  "x^{2} + y_{i}^{n} - e^{-x^2}",
  "\\sqrt{x^2 + 1} = \\sqrt[3]{8}",
  "\\gcd(a, b) = 1",
  "a \\bmod n",
  "a \\equiv b \\pmod{n}",
];
const RIGHT = [
  "\\left( \\frac{a}{b} \\right)",
  "\\sum_{i=1}^{n} i",
  "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}",
  "\\color{accent}{x} + \\color{#d1495b}{y}",
  "\\k{lhs}{2^4 - 1} = \\k{rhs}{15}",
  "\\overline{AB} + \\hat{x} + \\underbrace{a+b}_{2}",
];

const Column = ({ items }, { theme }) => (
  <Stack gap={18} style={{ flexGrow: 1, alignItems: "flex-start" }}>
    {items.map((tex, i) => (
      <Row key={`r${i}`} gap={16} align="baseline">
        <Text style={{ ...theme.type.caption, color: theme.color.muted }}>{String(i + 1)}</Text>
        <Math key={`m${i}`} tex={tex} style={{ fontSize: 30 }} />
      </Row>
    ))}
  </Stack>
);

export default (_p, { theme }) => (
  <Row gap={40} style={{ padding: 40, fill: theme.color.surface, height: "100%", alignItems: "flex-start" }}>
    <Column key="left" items={LEFT} />
    <Column key="right" items={RIGHT} />
  </Row>
);
