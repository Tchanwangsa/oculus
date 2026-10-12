import { useCallback, useState } from "react";
import { useNavigate } from "react-router-dom";

import type { StudioStyle } from "@/components/studio/style/style";
import { harnessSend } from "@/lib/harness";
import { videoHref } from "@/lib/studio/routes";
import { titleFromPrompt } from "@/lib/studio/title";
import { newVideo } from "@/lib/studio/videos";
import { readSetupProblems, type SetupProblem } from "@/lib/studio/setup";
import { useHarnessStore } from "@/stores/chat/harnessStore";
import { useVideosStore } from "@/stores/studio/videosStore";

/**
 * Starts a video: scaffolds its folder, then sends the first message with the
 * `studio` link so the thread is the video's. A video needs a configured
 * narration voice and the word-timing model (`setupProblems`), so those are
 * checked first and, lacking one, no folder is made: `problems` lists what to
 * fix in Settings. Resolves to false when that or either step fails, with
 * `error` set for a failure; the caller hands the words back to the box.
 */
export function useNewVideo(subjectId: number | null, subjectCode: string | null, style: StudioStyle) {
  const navigate = useNavigate();
  const [error, setError] = useState<string | null>(null);
  const [problems, setProblems] = useState<SetupProblem[]>([]);

  const start = useCallback(
    async (text: string): Promise<boolean> => {
      setError(null);
      setProblems([]);
      const store = useHarnessStore.getState();
      try {
        const missing = (await readSetupProblems()).problems;
        if (missing.length > 0) {
          setProblems(missing);
          return false;
        }
        const title = titleFromPrompt(text);
        const { slug } = await newVideo({
          title,
          subject: subjectCode,
          theme: style.theme ?? undefined,
          portrait: style.format === "reels",
        });
        await harnessSend(null, store.provider, text, {
          model: store.model,
          reasoningEffort: store.reasoning,
          subjectId,
          studio: { slug, lengthSecs: style.lengthSecs ?? undefined, format: style.format },
        });
        await Promise.all([store.loadThreads(), useVideosStore.getState().reload()]);
        navigate(videoHref(slug, title, true));
        return true;
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        return false;
      }
    },
    [navigate, subjectId, subjectCode, style],
  );

  return { start, error, problems };
}
