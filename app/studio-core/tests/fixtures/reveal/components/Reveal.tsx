export const props = { title: str(), body: str() };

export const examples = {
  short: { title: "Hello there", body: "One line." },
};

export default ({ title, body }, { t, theme }) => {
  const { color, space, type } = theme;
  return (
    <Stack justify="center" gap={space.xl} style={{ padding: [0, 160] }}>
      <Text
        style={{
          ...type.display,
          color: color.text,
          maxWidth: 1500,
          reveal: { by: "word", from: "rise", stagger: 0.6, progress: revealProgress(t, 0.2, 1.2) },
        }}
      >
        {title}
      </Text>
      <Text
        style={{
          ...type.body,
          fontSize: 40,
          color: color.muted,
          maxWidth: 1300,
          reveal: { by: "line", progress: revealProgress(t, 1.2, 0.8) },
        }}
      >
        {body}
      </Text>
    </Stack>
  );
};
