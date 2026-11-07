// title: A qubit's phase on the Bloch sphere
import { useEffect, useState } from "react";
import { Pill, Row, Slider, Tex, Value, Widget } from "oculus/ui";
import { useWidgetState } from "oculus/state";
import { Arc3D, Arrow3D, Axes3D, Curve3D, Label3D, Scene3D, Sphere, type Vec3 } from "oculus/3d";

// The mechanism: under H = (ω/2)σz the state turns about z at its latitude,
// so θ (the measurement odds) never changes and only the phase φ moves. At
// θ = 0 or π the turn is about the state itself: an eigenstate stays put.
const onSphere = (theta: number, phi: number): Vec3 => [
  Math.sin(theta) * Math.cos(phi),
  Math.sin(theta) * Math.sin(phi),
  Math.cos(theta),
];
const TURN = 2 * Math.PI;

export default function BlochPhase() {
  // Kept with the reply: a 3D scene's own state resets when it scrolls away.
  const [theta, setTheta] = useWidgetState("theta", 1.1);
  const [phi, setPhi] = useWidgetState("phi", 0);
  // Playing is not worth keeping: a reload should not start the animation.
  const [playing, setPlaying] = useState(false);

  useEffect(() => {
    if (!playing) return;
    let frame = 0;
    let last = performance.now();
    const tick = (now: number) => {
      setPhi((p) => (p + (now - last) / 1000) % TURN);
      last = now;
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [playing, setPhi]);

  const state = onSphere(theta, phi);
  const still = theta < 0.02 || theta > Math.PI - 0.02;
  return (
    <Widget>
      <Scene3D height={300}>
        <Sphere opacity={0.1} />
        <Sphere wire opacity={0.25} color="muted" />
        <Axes3D size={1.3} />
        <Label3D at={[0, 0, 1.2]} tex={"|0\\rangle"} attach="ne" />
        <Label3D at={[0, 0, -1.2]} tex={"|1\\rangle"} attach="se" />
        <Curve3D xyz={(s) => onSphere(theta, s)} domain={[0, TURN]} color="muted" weight={0.6} />
        <Arc3D from={[0, 0, 1]} to={state} radius={0.45} color="chart-2" />
        <Arrow3D tip={state} color={still ? "success" : "brand"} />
      </Scene3D>
      <Slider label={<Tex>{"\\theta"}</Tex>} value={theta} onChange={setTheta} min={0} max={Math.PI} step={0.01} digits={2} />
      <Slider label={<Tex>{"\\varphi"}</Tex>} value={phi} onChange={setPhi} min={0} max={TURN} step={0.01} digits={2} />
      <Row justify="between">
        <Row gap={16}>
          <Value label={<Tex>{"P(0)"}</Tex>} value={Math.cos(theta / 2) ** 2} digits={2} />
          <Value label={<Tex>{"P(1)"}</Tex>} value={Math.sin(theta / 2) ** 2} digits={2} />
        </Row>
        <Row>
          <Pill onClick={() => setTheta(0)}>Eigenstate</Pill>
          <Pill tone="primary" pressed={playing} onClick={() => setPlaying(!playing)}>
            {playing ? "Pause" : "Evolve"}
          </Pill>
        </Row>
      </Row>
    </Widget>
  );
}
