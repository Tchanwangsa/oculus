import { useEffect } from "react";

import { useStill, type Still } from "@/hooks/studio/useStill";
import { studioFrame, type SceneInfo, type VideoDetail } from "@/lib/studio/videos";

/**
 * A scene's still, `width` pixels wide: near the scene's end, where its
 * content has arrived, or a second in when there is no narration to say how
 * long it lasts. A still that cannot be made (a stale narration) reports its
 * sentence to `onProblem`.
 */
export function useSceneStill(
  detail: VideoDetail,
  index: number,
  scene: SceneInfo,
  width: number,
  onProblem: (message: string) => void,
): Still {
  const { slug, modified } = detail.row;
  const t = scene.duration != null ? scene.duration * 0.8 : 1;
  const still = useStill(`frame:${slug}:${modified}:${index}:${width}`, () =>
    studioFrame(slug, { scene: index, t }, width),
  );
  const failure = still.error;
  useEffect(() => {
    if (failure) onProblem(failure);
  }, [failure, onProblem]);
  return still;
}
