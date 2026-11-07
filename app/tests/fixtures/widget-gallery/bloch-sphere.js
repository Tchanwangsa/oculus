// title: A qubit's phase on the Bloch sphere
import { useEffect, useState } from "react";
import { Pill, Row, Slider, Tex, Value, Widget } from "oculus/ui";
import { useWidgetState } from "oculus/state";
import { Arc3D, Arrow3D, Axes3D, Curve3D, Label3D, Scene3D, Sphere } from "oculus/3d";
import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// The mechanism: under H = (ω/2)σz the state turns about z at its latitude,
// so θ (the measurement odds) never changes and only the phase φ moves. At
// θ = 0 or π the turn is about the state itself: an eigenstate stays put.
const onSphere = (theta, phi) => [
	Math.sin(theta) * Math.cos(phi),
	Math.sin(theta) * Math.sin(phi),
	Math.cos(theta)
];
const TURN = 2 * Math.PI;
function BlochPhase() {
	// Kept with the reply: a 3D scene's own state resets when it scrolls away.
	const [theta, setTheta] = useWidgetState("theta", 1.1);
	const [phi, setPhi] = useWidgetState("phi", 0);
	// Playing is not worth keeping: a reload should not start the animation.
	const [playing, setPlaying] = useState(false);
	useEffect(() => {
		if (!playing) return;
		let frame = 0;
		let last = performance.now();
		const tick = (now) => {
			setPhi((p) => (p + (now - last) / 1e3) % TURN);
			last = now;
			frame = requestAnimationFrame(tick);
		};
		frame = requestAnimationFrame(tick);
		return () => cancelAnimationFrame(frame);
	}, [playing, setPhi]);
	const state = onSphere(theta, phi);
	const still = theta < .02 || theta > Math.PI - .02;
	return _jsxs(Widget, { children: [
		_jsxs(Scene3D, {
			height: 300,
			children: [
				_jsx(Sphere, { opacity: .1 }),
				_jsx(Sphere, {
					wire: true,
					opacity: .25,
					color: "muted"
				}),
				_jsx(Axes3D, { size: 1.3 }),
				_jsx(Label3D, {
					at: [
						0,
						0,
						1.2
					],
					tex: "|0\\rangle",
					attach: "ne"
				}),
				_jsx(Label3D, {
					at: [
						0,
						0,
						-1.2
					],
					tex: "|1\\rangle",
					attach: "se"
				}),
				_jsx(Curve3D, {
					xyz: (s) => onSphere(theta, s),
					domain: [0, TURN],
					color: "muted",
					weight: .6
				}),
				_jsx(Arc3D, {
					from: [
						0,
						0,
						1
					],
					to: state,
					radius: .45,
					color: "chart-2"
				}),
				_jsx(Arrow3D, {
					tip: state,
					color: still ? "success" : "brand"
				})
			]
		}),
		_jsx(Slider, {
			label: _jsx(Tex, { children: "\\theta" }),
			value: theta,
			onChange: setTheta,
			min: 0,
			max: Math.PI,
			step: .01,
			digits: 2
		}),
		_jsx(Slider, {
			label: _jsx(Tex, { children: "\\varphi" }),
			value: phi,
			onChange: setPhi,
			min: 0,
			max: TURN,
			step: .01,
			digits: 2
		}),
		_jsxs(Row, {
			justify: "between",
			children: [_jsxs(Row, {
				gap: 16,
				children: [_jsx(Value, {
					label: _jsx(Tex, { children: "P(0)" }),
					value: Math.cos(theta / 2) ** 2,
					digits: 2
				}), _jsx(Value, {
					label: _jsx(Tex, { children: "P(1)" }),
					value: Math.sin(theta / 2) ** 2,
					digits: 2
				})]
			}), _jsxs(Row, { children: [_jsx(Pill, {
				onClick: () => setTheta(0),
				children: "Eigenstate"
			}), _jsx(Pill, {
				tone: "primary",
				pressed: playing,
				onClick: () => setPlaying(!playing),
				children: playing ? "Pause" : "Evolve"
			})] })]
		})
	] });
}

const __oculusWidget = BlochPhase;
