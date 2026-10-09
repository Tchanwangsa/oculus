import type { Dispatch, RefObject, SetStateAction } from "react";
import { FindBar } from "@/components/ui/search/FindBar";
import type { Find } from "@/components/files/pdf/types";

interface PdfFindBarProps {
  find: Find;
  setFind: Dispatch<SetStateAction<Find>>;
  inputRef: RefObject<HTMLInputElement | null>;
  runFind: (query: string, again: boolean, findPrevious?: boolean) => void;
  onClose: () => void;
}

/** The find row under the toolbar, wired to pdf.js's search. */
export function PdfFindBar({ find, setFind, inputRef, runFind, onClose }: PdfFindBarProps) {
  return (
    <FindBar
      inputRef={inputRef}
      query={find.query}
      onQueryChange={(query) => {
        setFind((f) => ({ ...f, query, status: undefined }));
        runFind(query, false);
      }}
      onStep={(backwards) => find.query && runFind(find.query, true, backwards)}
      onClose={onClose}
      status={find.status}
      placeholder="Find in PDF"
      className="shrink-0 border-b border-border-subtle bg-surface"
    />
  );
}
