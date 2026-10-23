import { createLibrary } from "@openuidev/react-lang";
import { Ask } from "./components/Ask";
import { Chart } from "./components/Chart";
import { Explorer, Slider, Value } from "./components/Explorer";
import { Card, Flashcards } from "./components/Flashcards";
import { PageRef } from "./components/PageRef";
import { Option, Question, Quiz } from "./components/Quiz";
import { Stack } from "./components/Stack";
import { Step, Steps } from "./components/Steps";
import { Text } from "./components/Text";
import { functions } from "./functions";

/**
 * The components an agent may use in an ```openui fence: a few study widgets,
 * not a layout kit. Every widget draws its own card, so the model picks what
 * to show and the app decides how it looks. Groups and their notes become
 * sections of the generated prompt (`prompt.ts`).
 */
export const library = createLibrary({
  root: "Stack",
  components: [Stack, Text, Flashcards, Card, Quiz, Question, Option, Steps, Step, Explorer, Slider, Value, Chart, PageRef, Ask],
  functions,
  componentGroups: [
    { name: "Root", components: ["Stack", "Text"] },
    {
      name: "Recall",
      components: ["Flashcards", "Card", "Quiz", "Question", "Option"],
      notes: [
        "- Mark exactly one answer per question with Option(text, true).",
        "- 5 to 12 cards or 3 to 8 questions; every one checks a single idea.",
      ],
    },
    { name: "Working", components: ["Steps", "Step"] },
    {
      name: "Explore",
      components: ["Explorer", "Slider", "Value", "Chart"],
      notes: [
        "- A Slider's first argument is a $variable declared with a literal default; Values and Charts compute from those $variables.",
        "- There is no ^ operator: write @Pow(base, exponent).",
      ],
    },
    { name: "Links", components: ["PageRef", "Ask"] },
  ],
});
