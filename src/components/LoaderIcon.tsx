/** Минималистичные SVG-иконки для загрузчиков */
export default function LoaderIcon({ loader, size = 20, className = "" }: { loader: string; size?: number; className?: string }) {
  const s = size;
  const cn = `shrink-0 ${className}`.trim();
  const base = { viewBox: "0 0 24 24", fill: "none", stroke: "currentColor", strokeWidth: 1.5, strokeLinecap: "round", strokeLinejoin: "round" } as const;

  switch (loader?.toLowerCase()) {
    case "vanilla":
      return (
        <svg {...base} width={s} height={s} className={cn}>
          <rect x="4" y="4" width="16" height="16" rx="2" />
          <path d="M9 12l3 3 3-3" />
        </svg>
      );
    case "fabric":
      return (
        <svg {...base} width={s} height={s} className={cn}>
          <path d="M4 4h6v6H4z" />
          <path d="M14 4h6v6h-6z" />
          <path d="M4 14h6v6H4z" />
          <path d="M14 14h6v6h-6z" />
        </svg>
      );
    case "quilt":
      return (
        <svg {...base} width={s} height={s} className={cn}>
          <rect x="2" y="2" width="20" height="20" rx="2" />
          <path d="M12 2v20" />
          <path d="M2 12h20" />
          <path d="M4 6l8 6 8-6" />
        </svg>
      );
    case "forge":
      return (
        <svg {...base} width={s} height={s} className={cn}>
          <path d="M12 3l-2 4h4l-2 4" />
          <rect x="4" y="11" width="16" height="8" rx="2" />
          <path d="M8 15h8" />
        </svg>
      );
    case "neoforge":
      return (
        <svg {...base} width={s} height={s} className={cn}>
          <path d="M12 2l-2 5 2 5 2-5-2-5z" />
          <rect x="4" y="12" width="16" height="8" rx="2" />
          <path d="M8 16h8" />
        </svg>
      );
    default:
      return (
        <svg {...base} width={s} height={s} className={cn}>
          <rect x="4" y="4" width="16" height="16" rx="2" />
          <path d="M9 12l3 3 3-3" />
        </svg>
      );
  }
}
