export const props = { step: num() };

// Step 1 -> 2 shares keys, so the default transition becomes a magic move:
// the chart moves and recolours, its bars regrow, the title's text changes
// (it swaps in halves), the badge leaves and the note arrives.
const BARS = [1, 0.7, 0.5, 0.36, 0.27];

export default ({ step }, { theme }) => {
  const { color } = theme;
  const two = step === 2;
  return (
    <Box style={{ fill: color.bg }}>
      <Box key="title" style={{ position: "absolute", left: 40, top: two ? 64 : 36, width: 400, height: two ? 44 : 56 }}>
        <Text style={{ fontFamily: "Manrope", fontSize: two ? 32 : 42, fontWeight: 700, letterSpacing: -1, color: color.text }}>{two ? "Step size matters" : "Gradient descent"}</Text>
      </Box>
      <Text key="sub" style={{ position: "absolute", left: 40, top: two ? 116 : 104, fontFamily: "Inter", fontSize: 15, color: color.muted }}>Lecture 4 · Optimisation</Text>
      {two ? null : (
        <Box key="badge" style={{ position: "absolute", right: 40, top: 40, padding: [6, 12], radius: 999, fill: color.accentSoft }}>
          <Text style={{ fontFamily: "Inter", fontSize: 13, fontWeight: 600, color: color.accentStrong }}>New</Text>
        </Box>
      )}
      <Row key="chart" gap={10} align="flex-end" style={{ position: "absolute", left: two ? 330 : 40, top: two ? 130 : 150, width: two ? 270 : 360, height: two ? 190 : 170, padding: 16, radius: two ? 12 : 24, fill: two ? color.ink : color.surface, shadow: { y: 10, blur: 30, color: color.shadow } }}>
        {BARS.map((h, i) => (
          <Box key={`bar${i}`} style={{ flexGrow: 1, height: `${(two ? 1.1 - h : h) * 100}%`, radius: 6, fill: two ? color.warning : color.accent }} />
        ))}
      </Row>
      {two ? (
        <Box key="note" style={{ position: "absolute", left: 40, top: 150, width: 250, padding: 16, radius: 12, fill: color.surface }}>
          <Text style={{ fontFamily: "Inter", fontSize: 16, color: color.text }}>Too large overshoots; too small crawls.</Text>
        </Box>
      ) : null}
    </Box>
  );
};
