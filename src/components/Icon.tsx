import type { CSSProperties } from "react";
const paths: Record<string, React.ReactNode> = {
  folder: (
    <>
      <path d="M3 6h6l2 2h10v11H3z" />
      <path d="M3 6V4h6l2 2h10v2" />
    </>
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  play: <path d="m8 5 11 7-11 7z" />,
  stop: <rect x="6" y="6" width="12" height="12" rx="1" />,
  restart: (
    <>
      <path d="M4 10a8 8 0 1 1 1 8M4 4v6h6" />
    </>
  ),
  terminal: (
    <>
      <path d="m5 7 5 5-5 5M12 17h7" />
    </>
  ),
  chevron: <path d="m9 5 7 7-7 7" />,
  ai: (
    <>
      <path d="m12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5z" />
    </>
  ),
  copy: (
    <>
      <rect x="8" y="8" width="12" height="13" rx="2" />
      <path d="M16 8V3H3v13h5" />
    </>
  ),
  search: (
    <>
      <circle cx="10" cy="10" r="6" />
      <path d="m15 15 5 5" />
    </>
  ),
  arrow: <path d="M12 4v16m-6-6 6 6 6-6" />,
  check: <path d="m5 12 4 4L19 6" />,
  close: <path d="m6 6 12 12M18 6 6 18" />,
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v3m0 14v3M2 12h3m14 0h3M5 5l2 2m10 10 2 2M5 19l2-2M17 7l2-2" />
    </>
  ),
  sun: (
    <>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 1v2m0 18v2M1 12h2m18 0h2M4 4l2 2m12 12 2 2M4 20l2-2M18 6l2-2" />
    </>
  ),
  shield: (
    <>
      <path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6z" />
      <path d="m8 12 3 3 5-6" />
    </>
  ),
};
export function Icon({
  name,
  size = 16,
  style,
}: {
  name: string;
  size?: number;
  style?: CSSProperties;
}) {
  return (
    <svg
      aria-hidden="true"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      style={style}
    >
      {paths[name] || paths.terminal}
    </svg>
  );
}
