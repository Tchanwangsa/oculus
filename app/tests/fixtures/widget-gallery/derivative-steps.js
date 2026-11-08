// title: Differentiate it step by step
import { Column, Field, Note, Pages, Row, Slider, Tex, Value, Widget } from "oculus/ui";
import { useWidgetState } from "oculus/state";
import { derivative, formula, parse } from "oculus/math";
import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
const d = (n) => derivative(n, "x").toTex();
const dx = (tex) => `\\frac{d}{dx}\\left(${tex}\\right)`;
function DerivativeSteps() {
	const [text, setText] = useWidgetState("text", "x^2 * sin(x)");
	const [page, setPage] = useWidgetState("page", 0);
	const [a, setA] = useWidgetState("a", 1);
	// Validate as the student types; an error stays inline and paging is refused.
	const f = formula(text);
	let steps = null;
	let error = f.ok ? null : f.error;
	if (f.ok) {
		try {
			const df = derivative(text, "x");
			steps = {
				rule: rule(text),
				raw: derivative(text, "x", { simplify: false }).toTex(),
				simple: df.toTex(),
				slope: (x) => {
					const y = df.evaluate({ x });
					return typeof y === "number" ? y : NaN;
				}
			};
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}
	const go = (next) => setPage(steps ? next : 0);
	const h = 1e-4;
	const secant = f.ok ? (f.fn(a + h) - f.fn(a - h)) / (2 * h) : NaN;
	return _jsx(Widget, { children: _jsxs(Pages, {
		page: steps ? page : 0,
		onPageChange: go,
		children: [
			_jsxs(Column, { children: [
				_jsx(Field, {
					label: "f(x)",
					value: text,
					onChange: setText,
					placeholder: "x^2 * sin(x)"
				}),
				error && _jsx(Note, {
					tone: "danger",
					children: error
				}),
				f.ok && _jsx(Tex, {
					display: true,
					children: `f(x) = ${f.tex}`
				})
			] }),
			_jsxs(Column, { children: [_jsx(Note, { children: steps?.rule.name }), _jsx(Tex, {
				display: true,
				children: steps?.rule.tex ?? ""
			})] }),
			_jsxs(Column, { children: [_jsx(Note, { children: "Put together" }), _jsx(Tex, {
				display: true,
				children: `f'(x) = ${steps?.raw ?? ""}`
			})] }),
			_jsxs(Column, { children: [
				_jsx(Note, { children: "Simplified, and checked against the slope" }),
				_jsx(Tex, {
					display: true,
					children: `f'(x) = ${steps?.simple ?? ""}`
				}),
				_jsx(Slider, {
					label: "x",
					value: a,
					onChange: setA,
					min: -3,
					max: 3,
					step: .1,
					digits: 1
				}),
				_jsxs(Row, {
					gap: 16,
					children: [_jsx(Value, {
						label: _jsx(Tex, { children: "f'(x)" }),
						value: steps ? steps.slope(a) : NaN,
						digits: 4
					}), _jsx(Value, {
						label: "Slope of a tiny secant",
						value: secant,
						digits: 4
					})]
				})
			] })
		]
	}) });
}
/** The rule the outermost operation calls for, written out with its parts. */
function rule(text) {
	let n = parse(text);
	while (n.type === "ParenthesisNode" && n.content) n = n.content;
	const [u, v] = n.args ?? [];
	if (n.type === "OperatorNode" && u && v) {
		const [ut, vt] = [u.toTex(), v.toTex()];
		if (n.op === "+" || n.op === "-") return {
			name: "Sum rule",
			tex: `${dx(ut)} ${n.op} ${dx(vt)} = ${d(u)} ${n.op} ${d(v)}`
		};
		if (n.op === "*") return {
			name: "Product rule",
			tex: `u'v + uv' = \\left(${d(u)}\\right)${vt} + ${ut}\\left(${d(v)}\\right)`
		};
		if (n.op === "/") return {
			name: "Quotient rule",
			tex: `\\frac{u'v - uv'}{v^2} = \\frac{\\left(${d(u)}\\right)${vt} - ${ut}\\left(${d(v)}\\right)}{\\left(${vt}\\right)^2}`
		};
		if (n.op === "^" && d(v) === "0") return {
			name: "Power rule",
			tex: `${vt}\\left(${ut}\\right)^{${vt}-1}\\cdot ${d(u)}`
		};
	}
	if (n.type === "FunctionNode" && u) return {
		name: "Chain rule",
		tex: `f'(g)\\,g' \\text{ with } g = ${u.toTex()},\\ g' = ${d(u)}`
	};
	return {
		name: "A rule to know",
		tex: `${dx(n.toTex())} = ${d(n)}`
	};
}

const __oculusWidget = DerivativeSteps;
