import { TopicPicker } from "./TopicPicker";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { updateItem, type RevisionItem } from "@/lib/revision";
export function ItemEditor({
  item,
  onClose,
}: {
  item: RevisionItem;
  onClose: () => void;
}) {
  const [topicIds, setTopicIds] = useState(item.topic_ids);
  const [prompt, setPrompt] = useState(item.prompt);
  const [answer, setAnswer] = useState(item.answer);
  const [level, setLevel] = useState(item.level);
  const [options, setOptions] = useState(item.options.join("\n"));
  const [rubric, setRubric] = useState(JSON.stringify(item.rubric, null, 2));
  const [tolerance, setTolerance] = useState(String(item.tolerance ?? 0));
  const [error, setError] = useState<string>();
  const save = async () => {
    try {
      const points: unknown = JSON.parse(rubric);
      if (
        !Array.isArray(points) ||
        points.some(
          (p) =>
            typeof p?.point !== "string" ||
            !p.point.trim() ||
            typeof p.marks !== "number" ||
            !Number.isFinite(p.marks) ||
            p.marks <= 0,
        )
      )
        throw new Error("Rubric points need text and positive marks");
      if (!prompt.trim() || !answer.trim())
        throw new Error("Question and answer must not be empty");
      await updateItem(item.id, {
        prompt,
        topic_ids: topicIds,
        answer,
        level,
        options: options.split("\n").filter(Boolean),
        rubric: points,
        tolerance: Number(tolerance),
      });
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div className="space-y-3 border-t border-border-subtle pt-3">
      <TopicPicker
        subjectId={item.subject_id}
        value={topicIds}
        onChange={setTopicIds}
      />
      <label className="block text-xs">
        Question
        <Textarea
          aria-label="Question"
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
        />
      </label>
      <label className="block text-xs">
        Model answer
        <Textarea
          aria-label="Model answer"
          value={answer}
          onChange={(e) => setAnswer(e.target.value)}
        />
      </label>
      <Select
        value={level}
        onValueChange={(v) => setLevel(v as RevisionItem["level"])}
      >
        <SelectTrigger aria-label="Difficulty">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {["easy", "medium", "hard"].map((v) => (
            <SelectItem key={v} value={v}>
              {v}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {item.kind === "choice" && (
        <Textarea
          aria-label="Options, one per line"
          value={options}
          onChange={(e) => setOptions(e.target.value)}
        />
      )}{" "}
      {item.kind === "numeric" && (
        <Input
          aria-label="Numeric tolerance"
          inputMode="decimal"
          value={tolerance}
          onChange={(e) => setTolerance(e.target.value)}
        />
      )}{" "}
      {item.kind === "structured" && (
        <label className="block text-xs">
          Rubric: point and marks
          <Textarea
            aria-label="Rubric JSON"
            value={rubric}
            onChange={(e) => setRubric(e.target.value)}
          />
        </label>
      )}
      {error && (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      )}
      <div className="flex gap-2">
        <Button size="sm" onClick={() => void save()}>
          Save
        </Button>
        <Button size="sm" variant="ghost" onClick={onClose}>
          Cancel
        </Button>
      </div>
    </div>
  );
}
