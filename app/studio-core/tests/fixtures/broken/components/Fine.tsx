export const props = { label: str() };
export const examples = { ok: { label: "x" }, bad: { label: 1 } };
export default ({ label }, { theme }) => (
  <Stack align="center" justify="center">
    <Image src="hills.png" style={{ width: 320 }} />
    <Text style={{ ...theme.type.h2 }}>{label}</Text>
  </Stack>
);
