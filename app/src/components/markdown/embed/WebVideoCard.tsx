import { useEffect } from "react";
import { Play } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { CardShell } from "@/components/markdown/embed/card/CardShell";
import { Thumbnail } from "@/components/markdown/embed/card/Thumbnail";
import { tagSource } from "@/components/markdown/embed/card/tagSource";
import { openExternal } from "@/lib/browser";
import { fmtDurationSecs } from "@/lib/lectures/format";
import {
  useWebVideos,
  webVideoInfo,
  webVideoKey,
  webVideoUrl,
  type WebVideoProvider,
} from "@/stores/lectures/webVideoStore";

const PROVIDERS: Record<WebVideoProvider, string> = { youtube: "YouTube", vimeo: "Vimeo" };

/**
 * A YouTube or Vimeo video embedded in a Canvas page (`<web-video>`, written
 * by the sync's HTML converter): thumbnail, title, channel and (Vimeo) length
 * from oEmbed, and Play, which opens the video's own page in a browser tab
 * (⌘-click: the system browser). Never an iframe here: YouTube refuses
 * embeds from the app's origin. Works without a subject, and with no
 * metadata the card still shows the tag's title and plays.
 */
export function WebVideoCard({
  provider,
  videoId,
  hash,
  title,
}: {
  provider?: string;
  videoId?: string;
  hash?: string;
  title?: string;
}) {
  if (!videoId || !(provider === "youtube" || provider === "vimeo")) return null;
  const h = provider === "vimeo" ? hash?.trim() || undefined : undefined;
  return <Card provider={provider} videoId={videoId} hash={h} title={title?.trim() || undefined} />;
}

/** `hash` is Vimeo's key to an unlisted video, which neither plays nor
 *  answers oEmbed without it. */
function Card({
  provider,
  videoId,
  hash,
  title,
}: {
  provider: WebVideoProvider;
  videoId: string;
  hash?: string;
  title?: string;
}) {
  const info = useWebVideos((s) => s.infos[webVideoKey(provider, videoId)]);

  useEffect(() => {
    void webVideoInfo(provider, videoId, hash).catch(() => {});
  }, [provider, videoId, hash]);

  const meta = info?.status === "ready" ? info.meta : null;
  const name = meta?.title || title || `${PROVIDERS[provider]} video`;
  const url = webVideoUrl(provider, videoId, hash);
  return (
    <CardShell
      md={tagSource("web-video", [["provider", provider], ["video-id", videoId], ["hash", hash], ["title", title]])}
      thumbnail={<Thumbnail key={meta?.thumbnail_url} src={meta?.thumbnail_url} />}
      title={name}
      meta={
        <>
          <span>{meta?.author_name || PROVIDERS[provider]}</span>
          {meta?.duration_s != null && (
            <span className="tabular-nums">{fmtDurationSecs(Math.round(meta.duration_s))}</span>
          )}
        </>
      }
      actions={
        <Button title={url} onClick={(e) => void openExternal(url, e.metaKey)}>
          <Play weight="fill" />
          Play
        </Button>
      }
    />
  );
}
