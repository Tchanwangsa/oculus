export const props = {};

// Bars of Math.random widths: the seed alone decides the picture.
export default ({}, { t }) => (
  <Stack gap={4} style={{ padding: 8, fill: "#ffffff" }}>
    {[0, 1, 2, 3, 4, 5].map((i) => (
      <Box key={i} style={{ height: 8, width: `${(0.2 + 0.8 * Math.random()) * t * 100}%`, fill: "#5e6ad2" }} />
    ))}
  </Stack>
);
