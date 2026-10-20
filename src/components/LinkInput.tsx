interface LinkInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  disabled?: boolean;
  singleLine: boolean;
}

/** Quiet multi-line input for one or more links. */
export function LinkInput({
  value,
  onChange,
  onSubmit,
  disabled,
  singleLine,
}: LinkInputProps) {
  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter") return;
    // Cmd+Enter always submits. A plain Enter submits only while the input
    // still holds a single line, so multi line editing keeps working.
    if (event.metaKey || event.ctrlKey || (singleLine && !event.shiftKey)) {
      event.preventDefault();
      onSubmit();
    }
  };

  return (
    <textarea
      aria-label="Link"
      placeholder="Paste link here..."
      spellCheck={false}
      autoCapitalize="off"
      autoCorrect="off"
      autoFocus
      disabled={disabled}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      onKeyDown={handleKeyDown}
      rows={4}
      className="w-full resize-none rounded-[10px] border border-border bg-surface px-3.5 py-3 text-[13px] leading-relaxed text-ink placeholder:text-muted/70 focus:border-accent focus:outline-none disabled:opacity-50"
    />
  );
}
