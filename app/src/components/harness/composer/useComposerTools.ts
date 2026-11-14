import { useCallback, useEffect, useState, type RefObject } from "react";
import type { MentionInputHandle } from "@/components/harness/mentions/MentionInput";
import { useComposerMenu } from "@/components/harness/mentions/useComposerMenu";
import { composerCommands, type ComposerCommandId } from "@/lib/harness/composerCommands";
import { useSkillStore } from "@/stores/chat/skillStore";

/** No command needs a bridge capability yet, so one list serves every box. */
const COMMANDS = composerCommands();

/**
 * What both composers (the Chat page's and the lecture dock's) share around
 * their box: the `@` and `/` menu, the skill catalogue it offers, and the
 * model picker `/model` opens.
 */
export function useComposerTools({
  subjectId,
  input,
}: {
  subjectId: number | null;
  input: RefObject<MentionInputHandle | null>;
}) {
  const skills = useSkillStore((s) => s.skills);
  const skillNames = useSkillStore((s) => s.names);
  const refreshSkills = useSkillStore((s) => s.refresh);
  useEffect(() => refreshSkills(), [refreshSkills]);

  const [modelOpen, setModelOpen] = useState(false);
  const onCommand = useCallback((id: ComposerCommandId) => {
    if (id === "model") setModelOpen(true);
  }, []);

  const menu = useComposerMenu({ subjectId, input, skills, commands: COMMANDS, onCommand });

  // Each `/` menu reads the folders again, so a skill installed meanwhile shows.
  const slash = menu.menu.kind === "slash";
  useEffect(() => {
    if (slash) refreshSkills();
  }, [slash, refreshSkills]);

  return { menu, skillNames, modelOpen, setModelOpen };
}
