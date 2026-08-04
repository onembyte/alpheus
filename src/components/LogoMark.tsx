/** The ring mark — open arcs, notch always facing the same corner. */
export default function LogoMark({ size }: { size: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      style={{ color: "var(--accent)", flex: "none" }}
    >
      <circle
        cx="50"
        cy="50"
        r="38"
        stroke="currentColor"
        strokeWidth="14"
        strokeLinecap="round"
        strokeDasharray="192.3 238.8"
        transform="rotate(-90 50 50)"
      />
      <circle
        cx="50"
        cy="50"
        r="20"
        stroke="currentColor"
        strokeWidth="12"
        strokeLinecap="round"
        strokeDasharray="83.8 125.7"
        transform="rotate(-90 50 50)"
        opacity=".6"
      />
    </svg>
  );
}
