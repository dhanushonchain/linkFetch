interface DownloadButtonProps {
  disabled: boolean;
  loading: boolean;
  label: string;
  onClick: () => void;
}

/** The single primary action of the app. */
export function DownloadButton({ disabled, loading, label, onClick }: DownloadButtonProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-busy={loading}
      className="w-full rounded-[10px] bg-accent px-4 py-2.5 text-[13px] font-medium text-[#171717] transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:bg-surface disabled:text-muted disabled:opacity-100"
    >
      {loading ? (
        <span className="flex items-center justify-center gap-2">
          <span
            aria-hidden="true"
            className="h-3 w-3 animate-spin rounded-full border-[1.5px] border-muted border-t-transparent"
          />
          {label}
        </span>
      ) : (
        label
      )}
    </button>
  );
}
