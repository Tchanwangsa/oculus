export const props = {};
export default ({}, { t, duration }) => {
  if (t > duration / 2) throw new Error("ran past the halfway mark");
  return <Box />;
};
