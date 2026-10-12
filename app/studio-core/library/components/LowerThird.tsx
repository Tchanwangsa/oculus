export const props = {
  name: str(),
  role: str().optional(),
  accent: token("color").default("accent"),
};

export const examples = {
  speaker: { name: "Ada Lovelace", role: "Lecturer, Analytical Engines" },
  name: { name: "Ada Lovelace" },
};

export default ({ name, role, accent }, { theme }) => {
  const { color, space, radius, type } = theme;
  return (
    <Box style={{ flexGrow: 1, fill: color.bg }}>
      <Row align="stretch" style={{ position: "absolute", left: 96, bottom: 96, shadow: { y: 12, blur: 40, color: color.shadow } }}>
        <Box style={{ width: 10, radius: radius.s, fill: accent }} />
        <Stack gap={space.xxs} style={{ padding: [space.s, space.l], radius: radius.s, fill: color.surface }}>
          <Text style={{ ...type.h3, color: color.text }}>{name}</Text>
          {role ? <Text style={{ ...type.caption, color: color.muted }}>{role}</Text> : null}
        </Stack>
      </Row>
    </Box>
  );
};
