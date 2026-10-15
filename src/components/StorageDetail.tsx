import type { Row, Screen } from "../breakdown/state";
import { categoryColor, statusLine } from "../breakdown/present";
import { fmtBytes } from "../format";

interface Props {
  screen: Screen;
}

function Spinner() {
  return (
    <svg className="spin-fast flex-none" width="10" height="10" viewBox="0 0 12 12" fill="none">
      <circle cx="6" cy="6" r="4.5" stroke="var(--track)" strokeWidth="1.6" />
      <path
        d="M6 1.5a4.5 4.5 0 0 1 4.5 4.5"
        stroke="var(--accent)"
        strokeWidth="1.6"
        strokeLinecap="round"
      />
    </svg>
  );
}

/**
 * What the disk holds, by the categories System Settings → Storage uses,
 * measured live by the backend. Read-only: removing things is the cards' job.
 */
export default function StorageDetail({ screen: s }: Props) {
  const max = Math.max(1, ...s.rows.map((r) => Math.max(r.bytes, r.previous ?? 0)));

  return (
    <section className="glass-card fade-up px-[18px] pb-1.5 pt-4">
      <div className="mb-3 flex items-baseline gap-2.5">
        <h2 className="section-label flex-none">Storage detail</h2>
        {s.total > 0 && (
          <span className="mono flex-none text-[10.5px]" style={{ color: "var(--txt2)" }}>
            {fmtBytes(s.total)} used
          </span>
        )}
        <span
          className="mono ml-auto flex min-w-0 items-center gap-1.5 self-center text-[10px]"
          style={{ color: "var(--txt3)" }}
        >
          {s.running && <Spinner />}
          <span className="truncate">{statusLine(s)}</span>
        </span>
      </div>

      {s.total > 0 && (
        <div
          className="relative flex h-[14px] gap-[2px] overflow-hidden rounded-[7px]"
          style={{ background: "var(--track)", boxShadow: "inset 0 1px 3px rgba(0,0,0,.22)" }}
        >
          {s.rows.map(
            (r) =>
              r.bytes > 0 && (
                <div
                  key={r.id}
                  title={`${r.name} — ${fmtBytes(r.bytes)}`}
                  style={{
                    width: `${r.share * 100}%`,
                    background: categoryColor(r.name),
                    transition: "width .5s ease",
                  }}
                />
              ),
          )}
          <div
            className="pointer-events-none absolute inset-0"
            style={{
              background:
                "linear-gradient(180deg, rgba(255,255,255,.42), rgba(255,255,255,.06) 48%, rgba(0,0,0,.10))",
            }}
          />
        </div>
      )}

      <div className="mt-1.5">
        {s.rows.map((r) => (
          <DetailRow key={r.id} row={r} max={max} />
        ))}
        {s.rows.length === 0 && (
          <div className="py-6 text-center text-[12px]" style={{ color: "var(--txt3)" }}>
            {s.loading ? "Opening…" : "Measuring starts in a moment."}
          </div>
        )}
      </div>
    </section>
  );
}

function DetailRow({ row: r, max }: { row: Row; max: number }) {
  const color = categoryColor(r.name);
  const measuring = r.state !== "done";
  return (
    <div className="hairline-b flex items-center gap-3.5 py-2 last:border-0">
      <div className="h-[22px] w-2 flex-none rounded-[3px]" style={{ background: color }} />
      <div className="w-[190px] flex-none">
        <div className="truncate text-[12.5px] font-medium" style={{ color: "var(--txt)" }}>
          {r.name}
        </div>
        <div className="mono truncate text-[9.5px]" style={{ color: "var(--txt3)" }}>
          {measuring
            ? r.previous !== null
              ? `measuring · ${fmtBytes(r.previous)} last time`
              : "measuring"
            : `${(r.share * 100).toFixed(r.share < 0.1 ? 1 : 0)}% of used`}
        </div>
      </div>
      <div
        className="relative h-[9px] min-w-0 flex-1 overflow-hidden rounded-[5px]"
        style={{ background: "var(--track)" }}
      >
        {measuring && r.previous !== null && (
          <div
            className="absolute inset-y-0 left-0 rounded-[5px]"
            style={{
              width: `${(r.previous / max) * 100}%`,
              background: `color-mix(in srgb, ${color} 22%, transparent)`,
            }}
          />
        )}
        {r.bytes > 0 && (
          <div
            className="relative h-full rounded-[5px]"
            style={{
              width: `${(r.bytes / max) * 100}%`,
              background: `linear-gradient(90deg, ${color}, color-mix(in oklch, ${color}, white 22%))`,
              transition: "width .5s ease",
            }}
          />
        )}
      </div>
      <div className="flex w-[86px] flex-none items-center justify-end gap-1.5">
        {measuring && <Spinner />}
        <span
          className="mono text-right text-[12px] font-semibold"
          style={{ color: measuring ? "var(--txt2)" : "var(--txt)" }}
        >
          {r.bytes > 0 ? fmtBytes(r.bytes) : "—"}
        </span>
      </div>
    </div>
  );
}
