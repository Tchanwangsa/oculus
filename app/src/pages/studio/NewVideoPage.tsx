import { NewVideo } from "@/components/studio/new/NewVideo";
import { StudioHeader } from "@/components/studio/StudioHeader";

/** /studio: describe a video and start its thread. */
export default function NewVideoPage() {
  return (
    <>
      <StudioHeader title="New video" />
      <NewVideo />
    </>
  );
}
