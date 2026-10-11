import { MagnifyingGlass, X } from "@phosphor-icons/react";

/** The pill search field. Enter searches; `MusicLibrary` also searches after
 *  a pause in typing, never per keystroke, since every search is a call to
 *  Spotify, which can rate-limit Oculus. Escape, ×, or emptying the field
 *  clears. */
export function LibrarySearch({
  value,
  onChange,
  onSearch,
  onClear,
  disabled,
}: {
  value: string;
  onChange: (value: string) => void;
  onSearch: () => void;
  onClear: () => void;
  disabled: boolean;
}) {
  return (
    <div className="flex h-8 items-center gap-2 rounded-full border border-input bg-card px-3 transition-[color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/25">
      <MagnifyingGlass size={13} className="shrink-0 text-muted-foreground" />
      <input
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            onSearch();
          } else if (e.key === "Escape" && value) {
            e.preventDefault();
            onClear();
          }
        }}
        placeholder="Search Spotify"
        aria-label="Search Spotify"
        spellCheck={false}
        autoComplete="off"
        className="min-w-0 flex-1 bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground disabled:opacity-50"
      />
      {value && (
        <button
          type="button"
          aria-label="Clear search"
          onClick={onClear}
          className="shrink-0 cursor-pointer rounded-full p-0.5 text-muted-foreground hover:text-foreground"
        >
          <X size={12} />
        </button>
      )}
    </div>
  );
}
