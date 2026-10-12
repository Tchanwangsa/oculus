export const props = {};

const Card = ({ label }, { theme }) => {
  const { color, space, radius, type } = theme;
  return (
    <Row align="center" style={{ height: 88, padding: [0, space.l], radius: radius.m, fill: color.surface, border: { width: 2, color: color.border }, shadow: { y: 10, blur: 30, color: color.shadow } }}>
      <Text style={{ ...type.h3, color: color.text }}>{label}</Text>
    </Row>
  );
};

const presets = ["fade", "rise", "slide-left", "slide-right", "scale", "blur", "mask"];

export default ({}, { theme }) => {
  const { color, space } = theme;
  return (
    <Row gap={space.xl} align="center" justify="center" style={{ fill: color.surfaceAlt }}>
      <Stack gap={space.s} style={{ width: 640 }}>
        {presets.map((p) => (
          <Enter key={p} preset={p} at={0.25} dur={1} easing={easeInOutCubic}>
            <Card label={p} />
          </Enter>
        ))}
      </Stack>
      <Stack gap={space.s} style={{ width: 640 }}>
        <Card label="before" />
        <Sequence from={1.5}>
          <Card label="hidden, slot kept" />
        </Sequence>
        <Card label="after" />
        <Exit preset="rise" at={0.5} dur={0.5} easing={linear}>
          <Card label="exit rise" />
        </Exit>
        <Stagger each={0.25}>
          <Enter preset="slide-left" dur={0.5} easing={linear}><Card label="stagger 0" /></Enter>
          <Enter preset="slide-left" dur={0.5} easing={linear}><Card label="stagger 1" /></Enter>
          <Enter preset="slide-left" dur={0.5} easing={linear}><Card label="stagger 2" /></Enter>
        </Stagger>
      </Stack>
    </Row>
  );
};
