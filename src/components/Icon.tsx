const PATHS = {
  chevron: "M3.5 6l4.5 4.5L12.5 6",
  check: "M3 8.5l3.2 3.2L13 4.5",
  trash: "M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.5h5.8l.6-8.5",
  plus: "M8 3v10M3 8h10",
  up: "M8 13V3.5M4 7l4-3.5L12 7",
  down: "M8 3v9.5M4 9l4 3.5L12 9",
  close: "M4 4l8 8M12 4L4 12",
  undo: "M6 3.5L3 6.5l3 3M3 6.5h6a3.5 3.5 0 010 7H6",
  reset: "M13 8a5 5 0 11-1.5-3.5M12 2v3H9",
  grip: "M6 4v.01M6 8v.01M6 12v.01M10 4v.01M10 8v.01M10 12v.01",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 14 }: { name: IconName; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={name === "grip" ? 2 : 1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
