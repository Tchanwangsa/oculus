import { useEffect } from "react";
import { Section } from "@/components/settings/shared/section";
import { SettingRow } from "@/components/settings/shared/SettingRow";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { FollowUp } from "@/lib/harness";
import { shortcut } from "@/lib/platform";
import { useChatPrefsStore } from "@/stores/chat/chatPrefsStore";

/**
 * What Enter does with a message sent while the agent works (`useSendMode`):
 * steer the running turn, or queue behind it. ⌘↩ always does the other, and
 * an agent that cannot take a message mid-turn queues either way.
 */
export function FollowUpSection() {
  const { followUp, loaded, load, setFollowUp } = useChatPrefsStore();
  useEffect(() => {
    if (!loaded) void load().catch(() => {});
  }, [loaded, load]);

  return (
    <Section title="Chat" description="How the composer sends while the agent is working.">
      <SettingRow
        label="While the agent works"
        hint={`Enter does this; ${shortcut("mod+enter")} does the other. An agent that can't take a message mid-turn queues it either way.`}
      >
        <Select value={followUp} onValueChange={(next) => void setFollowUp(next as FollowUp)}>
          <SelectTrigger aria-label="While the agent works" size="sm" className="h-7 w-48 text-xs">
            <SelectValue placeholder="—" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="steer" className="text-xs">
              Steer the current turn
            </SelectItem>
            <SelectItem value="queue" className="text-xs">
              Queue for after it
            </SelectItem>
          </SelectContent>
        </Select>
      </SettingRow>
    </Section>
  );
}
