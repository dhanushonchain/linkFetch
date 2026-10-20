import { TARGETS, type Target } from "../lib/validation";

interface TargetSelectProps {
  value: Target;
  onChange: (target: Target) => void;
  disabled?: boolean;
}

/** Compact dropdown that picks what should be fetched from the link. */
export function TargetSelect({ value, onChange, disabled }: TargetSelectProps) {
  return (
    <div className="relative">
      <select
        aria-label="Target"
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value as Target)}
        className="w-full appearance-none rounded-[10px] border border-border bg-surface py-2 pl-3 pr-8 text-[13px] text-ink focus:border-accent focus:outline-none disabled:opacity-50"
      >
        {TARGETS.map((target) => (
          <option key={target.value} value={target.value}>
            {target.label}
          </option>
        ))}
      </select>
      <svg
        aria-hidden="true"
        viewBox="0 0 10 6"
        className="pointer-events-none absolute right-3 top-1/2 h-1.5 w-2.5 -translate-y-1/2 text-muted"
      >
        <path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    </div>
  );
}
