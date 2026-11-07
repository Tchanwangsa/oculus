// title: Bubble sort, one comparison at a time
import { useEffect, useMemo, useState } from "react";
import { Pill, Row, Slider, Value, Widget } from "oculus/ui";
import { useWidgetState } from "oculus/state";
import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
function bubbleSort(start) {
	const a = [...start];
	const frames = [{
		values: [...a],
		pair: null,
		swapped: false,
		sortedFrom: a.length,
		comparisons: 0,
		swaps: 0
	}];
	let comparisons = 0;
	let swaps = 0;
	for (let end = a.length - 1; end > 0; end--) {
		for (let j = 0; j < end; j++) {
			comparisons++;
			const swapped = a[j] > a[j + 1];
			if (swapped) {
				[a[j], a[j + 1]] = [a[j + 1], a[j]];
				swaps++;
			}
			frames.push({
				values: [...a],
				pair: [j, j + 1],
				swapped,
				sortedFrom: end + 1,
				comparisons,
				swaps
			});
		}
	}
	frames.push({
		values: [...a],
		pair: null,
		swapped: false,
		sortedFrom: 0,
		comparisons,
		swaps
	});
	return frames;
}
// Colour is state, never position: compared, just swapped, in place, waiting.
function barColour(frame, k) {
	if (k >= frame.sortedFrom) return "var(--color-success)";
	if (frame.pair?.includes(k)) return frame.swapped ? "var(--color-warning)" : "var(--color-brand)";
	return "var(--color-muted-foreground)";
}
const shuffled = () => Array.from({ length: 8 }, () => 1 + Math.floor(Math.random() * 9));
function BubbleSort() {
	const [start, setStart] = useWidgetState("start", [
		5,
		2,
		8,
		1,
		9,
		3,
		7,
		4
	]);
	const [step, setStep] = useWidgetState("step", 0);
	const [playing, setPlaying] = useState(false);
	const frames = useMemo(() => bubbleSort(start), [start]);
	const last = frames.length - 1;
	const frame = frames[Math.min(step, last)];
	useEffect(() => {
		if (!playing) return;
		const id = setInterval(() => setStep((s) => Math.min(s + 1, last)), 450);
		return () => clearInterval(id);
	}, [
		playing,
		last,
		setStep
	]);
	useEffect(() => {
		if (step >= last) setPlaying(false);
	}, [step, last]);
	return _jsxs(Widget, { children: [
		_jsx("div", {
			role: "img",
			"aria-label": `Array ${frame.values.join(", ")}`,
			style: {
				display: "flex",
				alignItems: "flex-end",
				gap: 6,
				height: 160
			},
			children: frame.values.map((v, k) => _jsx("div", { style: {
				flex: 1,
				height: `${v / 9 * 100}%`,
				borderRadius: 4,
				background: barColour(frame, k)
			} }, k))
		}),
		_jsx(Slider, {
			label: "Step",
			value: Math.min(step, last),
			onChange: setStep,
			min: 0,
			max: last,
			step: 1
		}),
		_jsxs(Row, {
			justify: "between",
			children: [_jsxs(Row, {
				gap: 16,
				children: [_jsx(Value, {
					label: "Comparisons",
					value: frame.comparisons
				}), _jsx(Value, {
					label: "Swaps",
					value: frame.swaps
				})]
			}), _jsxs(Row, { children: [
				_jsx(Pill, {
					onClick: () => setStep(Math.max(0, step - 1)),
					disabled: step === 0,
					children: "Back"
				}),
				_jsx(Pill, {
					onClick: () => setStep(Math.min(last, step + 1)),
					disabled: step >= last,
					children: "Next"
				}),
				_jsx(Pill, {
					tone: "primary",
					pressed: playing,
					onClick: () => setPlaying(!playing),
					disabled: step >= last,
					children: playing ? "Pause" : "Play"
				}),
				_jsx(Pill, {
					tone: "ghost",
					onClick: () => {
						setStart(shuffled());
						setStep(0);
					},
					children: "Shuffle"
				})
			] })]
		})
	] });
}

const __oculusWidget = BubbleSort;
