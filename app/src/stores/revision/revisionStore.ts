import { create } from "zustand";
import {
  getNotes,
  getTopics,
  type RevisionTopic,
  getItems,
  getSets,
  getArtifacts,
  type RevisionNote,
  type RevisionItem,
  type RevisionSet,
  type RevisionArtifact,
} from "@/lib/revision";
interface RevisionState {
  topics: RevisionTopic[];
  notes: RevisionNote[];
  items: RevisionItem[];
  sets: RevisionSet[];
  artifacts: RevisionArtifact[];
  loading: boolean;
  error: string | null;
  reload: () => Promise<void>;
}
let generation = 0;
export const useRevisionStore = create<RevisionState>((set) => ({
  topics: [],
  notes: [],
  items: [],
  sets: [],
  artifacts: [],
  loading: false,
  error: null,
  reload: async () => {
    const request = ++generation;
    set({ loading: true, error: null });
    try {
      const [notes, items, sets, artifacts, topics] = await Promise.all([
        getNotes(),
        getItems(),
        getSets(),
        getArtifacts(),
        getTopics(),
      ]);
      if (generation === request)
        set({ notes, items, sets, artifacts, topics, loading: false });
    } catch (error) {
      if (generation === request) set({ loading: false, error: String(error) });
    }
  },
}));
