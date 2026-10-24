import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { getLecture, getChapters } from "@/lib/db";
import { lecturePagePath, LECTURE_CHAPTERS_EVENT } from "@/lib/lectures";
import { videoDetail, studioThumbnail } from "@/lib/studio/videos";
import { videoHref } from "@/lib/studio/routes";
import { useStill } from "@/hooks/studio/useStill";
import { Frame } from "./Frame";
import { useLiveData } from "./useLiveData";

export function playbackLabel(seconds: number) {
  const at = Math.max(0, Math.floor(seconds));
  return `${Math.floor(at / 60)}:${String(at % 60).padStart(2, "0")}`;
}
export const Lecture = defineComponent({
  name: "Lecture",
  description:
    "A lecture's live title and chapter, with a link to play from at seconds. Read its id with the CLI.",
  props: z.object({ id: z.string(), at: z.number().nonnegative().nullish() }),
  component: function LectureView({ props }) {
    const data = useLiveData(
      `lecture:${props.id}`,
      async () => {
        const [lecture, chapters] = await Promise.all([
          getLecture(props.id),
          getChapters(props.id),
        ]);
        return { lecture, chapters };
      },
      [LECTURE_CHAPTERS_EVENT, "oculus:calendar-updated"],
    );
    const [thumb, setThumb] = useState<string | null>(null);
    useEffect(() => {
      let stale = false;
      setThumb(null);
      void invoke<string | null>("lecture_thumbnail", { lectureId: props.id })
        .then((path) => {
          if (!stale) setThumb(path ? convertFileSrc(path) : null);
        })
        .catch(() => {});
      return () => {
        stale = true;
      };
    }, [props.id]);
    const lecture = data.value?.lecture;
    const chapter = data.value?.chapters
      .filter((c) => c.start_seconds <= (props.at ?? 0))
      .slice(-1)[0];
    return (
      <Frame title={lecture?.title ?? "Lecture"}>
        {lecture ? (
          <Link
            to={lecturePagePath(lecture, props.at ?? undefined)}
            className="block space-y-2 hover:text-brand"
          >
            {thumb && (
              <img
                alt=""
                src={thumb}
                className="aspect-video w-full rounded-lg object-cover"
              />
            )}
            {chapter && (
              <p className="text-sm text-muted-foreground">{chapter.title}</p>
            )}
            <span className="text-sm">
              {props.at != null
                ? `Play from ${playbackLabel(props.at)}`
                : "Open lecture"}
            </span>
          </Link>
        ) : (
          <p className="text-sm text-muted-foreground">
            {data.error ??
              (data.value
                ? "This lecture is no longer available."
                : "Loading lecture…")}
          </p>
        )}
      </Frame>
    );
  },
});
export const StudioVideo = defineComponent({
  name: "StudioVideo",
  description:
    "A Studio video's live title and poster, opening its player. Read the slug from studio list.",
  props: z.object({ slug: z.string() }),
  component: function StudioVideoView({ props }) {
    const data = useLiveData(
      `studio:${props.slug}`,
      () => videoDetail(props.slug),
      ["oculus:studio-updated"],
    );
    const video = data.value?.row;
    const still = useStill(
      video ? `thumb:${video.slug}:${video.modified}` : null,
      () => studioThumbnail(props.slug),
    );
    return (
      <Frame title={video?.title ?? "Studio video"}>
        {video ? (
          <Link
            to={videoHref(video.slug, video.title)}
            className="block space-y-2 text-sm hover:text-brand"
          >
            {still.url && (
              <img
                src={still.url}
                alt=""
                className="aspect-video w-full rounded-lg object-contain"
              />
            )}
            <span>
              Open video
              {video.scenes != null ? ` · ${video.scenes} scenes` : ""}
            </span>
          </Link>
        ) : (
          <p className="text-sm text-muted-foreground">
            {data.error ?? "Loading video…"}
          </p>
        )}
      </Frame>
    );
  },
});
