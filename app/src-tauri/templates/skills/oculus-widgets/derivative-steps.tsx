// title: Differentiate it step by step
import { Column, Field, Note, Pages, Row, Slider, Tex, Value, Widget } from "oculus/ui";
import { useWidgetState } from "oculus/state";
import { derivative, formula, parse, type Node } from "oculus/math";

const d = (n: Node) => derivative(n, "x").toTex();
const dx = (tex: string) => `\\frac{d}{dx}\\left(${tex}\\right)`;

/** The rule the outermost operation calls for, written out with its parts. */
function rule(text: string): { name: string; tex: string } {
  let n = parse(text);
  while (n.type === "ParenthesisNode" && n.content) n = n.content;
  const [u, v] = n.args ?? [];
  if (n.type === "OperatorNode" && u && v) {
    const [ut, vt] = [u.toTex(), v.toTex()];
    if (n.op === "+" || n.op === "-")
      return { name: "Sum rule", tex: `${dx(ut)} ${n.op} ${dx(vt)} = ${d(u)} ${n.op} ${d(v)}` };
    if (n.op === "*")
      return { name: "Product rule", tex: `u'v + uv' = \\left(${d(u)}\\right)${vt} + ${ut}\\left(${d(v)}\\right)` };
    if (n.op === "/")
      return { name: "Quotient rule", tex: `\\frac{u'v - uv'}{v^2} = \\frac{\\left(${d(u)}\\right)${vt} - ${ut}\\left(${d(v)}\\right)}{\\left(${vt}\\right)^2}` };
    if (n.op === "^" && d(v) === "0")
      return { name: "Power rule", tex: `${vt}\\left(${ut}\\right)^{${vt}-1}\\cdot ${d(u)}` };
  }
  if (n.type === "FunctionNode" && u)
    return { name: "Chain rule", tex: `f'(g)\\,g' \\text{ with } g = ${u.toTex()},\\ g' = ${d(u)}` };
  return { name: "A rule to know", tex: `${dx(n.toTex())} = ${d(n)}` };
}

export default function DerivativeSteps() {
  const [text, setText] = useWidgetState("text", "x^2 * sin(x)");
  const [page, setPage] = useWidgetState("page", 0);
  const [a, setA] = useWidgetState("a", 1);

  // Validate as the student types; an error stays inline and paging is refused.
  const f = formula(text);
  let steps: { rule: { name: string; tex: string }; raw: string; simple: string; slope: (x: number) => number } | null = null;
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
        },
      };
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
  const go = (next: number) => setPage(steps ? next : 0);
  const h = 1e-4;
  const secant = f.ok ? (f.fn(a + h) - f.fn(a - h)) / (2 * h) : NaN;

  return (
    <Widget>
      <Pages page={steps ? page : 0} onPageChange={go}>
        <Column>
          <Field label="f(x)" value={text} onChange={setText} placeholder="x^2 * sin(x)" />
          {error && <Note tone="danger">{error}</Note>}
          {f.ok && <Tex display>{`f(x) = ${f.tex}`}</Tex>}
        </Column>
        <Column>
          <Note>{steps?.rule.name}</Note>
          <Tex display>{steps?.rule.tex ?? ""}</Tex>
        </Column>
        <Column>
          <Note>Put together</Note>
          <Tex display>{`f'(x) = ${steps?.raw ?? ""}`}</Tex>
        </Column>
        <Column>
          <Note>Simplified, and checked against the slope</Note>
          <Tex display>{`f'(x) = ${steps?.simple ?? ""}`}</Tex>
          <Slider label="x" value={a} onChange={setA} min={-3} max={3} step={0.1} digits={1} />
          <Row gap={16}>
            <Value label={<Tex>{"f'(x)"}</Tex>} value={steps ? steps.slope(a) : NaN} digits={4} />
            <Value label="Slope of a tiny secant" value={secant} digits={4} />
          </Row>
        </Column>
      </Pages>
    </Widget>
  );
}
