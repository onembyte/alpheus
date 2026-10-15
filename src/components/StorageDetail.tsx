import type { Crumb, Row, Screen } from "../breakdown/state";
import type { Link } from "../breakdown/links";
import { accessNotice } from "../breakdown/links";
import { categoryColor, statusLine } from "../breakdown/present";
import { fmtBytes, fmtKB } from "../format";
import type { Card } from "../types";

interface Props {
  screen: Screen;
  /** Opens a row; the walk keeps running meanwhile. */
  onOpen: (crumb: Crumb) => void;
  /** Back to the breadcrumb at `to`; -1 is the top. */
  onUp: (to: number) => void;
  /** Reclaim cards each row holds entirely, by row id. */
  links: Map<number, Link>;
  /** Opens a card's review sheet: the only way anything here gets removed. */
  onReclaim: (card: Card) => void;
  onFullDiskAccess: () => void;
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

function Chevron({ left = false }: { left?: boolean }) {
  return (
    <svg
      width="10"
      height="10"
      viewBox="0 0 12 12"
      fill="none"
      style={{ transform: left ? "rotate(180deg)" : "none" }}
    >
      <path
        d="M4.4 2.4 8 6l-3.6 3.6"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

/**
 * What the disk holds, by the categories System Settings → Storage uses,
 * measured live by the backend. Rows open while the walk runs. Read-only:
 * removing things is the cards' job.
 */
export default function StorageDetail({
  screen: s,
  onOpen,
  onUp,
  links,
  onReclaim,
  onFullDiskAccess,
}: Props) {
  const notice = accessNotice(s.unreadable);
  const top = s.crumbs.length === 0;
  // Inside a category every row wears the category's color.
  const colorOf = (r: Row) => categoryColor(top ? r.name : s.crumbs[0].name);
  const max = Math.max(
    1,
    s.filesHere ?? 0,
    ...s.rows.map((r) => Math.max(r.bytes, r.measured)),
  );

  return (
    <section className="glass-card fade-up px-[18px] pb-1.5 pt-4">
      <div className="mb-3 flex min-w-0 items-center gap-2.5">
        {!top && (
          <button
            onClick={() => onUp(s.crumbs.length - 2)}
            aria-label="Back"
            className="glass-chip btn-focus flex h-6 w-6 flex-none items-center justify-center rounded-md"
            style={{ color: "var(--txt2)" }}
          >
            <Chevron left />
          </button>
        )}
        <nav className="flex min-w-0 items-baseline gap-1.5" aria-label="Breadcrumb">
          <button
            onClick={() => onUp(-1)}
            disabled={top}
            className="section-label btn-focus flex-none"
            style={{ cursor: top ? "default" : "pointer" }}
          >
            Storage detail
          </button>
          {s.crumbs.map((c, i) => (
            <span key={c.id} className="flex min-w-0 items-baseline gap-1.5">
              <span className="text-[11px]" style={{ color: "var(--txt3)" }}>
                ›
              </span>
              <button
                onClick={() => onUp(i)}
                disabled={i === s.crumbs.length - 1}
                className="btn-focus truncate text-[12px] font-semibold"
                style={{ color: i === s.crumbs.length - 1 ? "var(--txt)" : "var(--txt2)" }}
              >
                {c.name}
              </button>
            </span>
          ))}
        </nav>
        {s.total > 0 && (
          <span className="mono flex-none text-[10.5px]" style={{ color: "var(--txt2)" }}>
            {fmtBytes(s.total)}
            {top ? " used" : ""}
          </span>
        )}
        <span
          className="mono ml-auto flex min-w-0 items-center gap-1.5 text-[10px]"
          style={{ color: "var(--txt3)" }}
        >
          {(s.running || s.measuringHere) && <Spinner />}
          <span className="truncate">{statusLine(s)}</span>
        </span>
      </div>

      {s.total > 0 && (
        <div
          className="relative flex h-[14px] gap-[2px] overflow-hidden rounded-[7px]"
          style={{ background: "var(--track)", boxShadow: "inset 0 1px 3px rgba(0,0,0,.22)" }}
        >
          {s.rows.map(
            (r, i) =>
              r.bytes > 0 && (
                <div
                  key={r.id}
                  title={`${r.name} — ${fmtBytes(r.bytes)}`}
                  style={{
                    width: `${r.share * 100}%`,
                    background: colorOf(r),
                    // Inside a category, fade down the list so neighbours part.
                    opacity: top ? 1 : Math.max(0.35, 1 - i * 0.12),
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
          <DetailRow
            key={r.id}
            row={r}
            max={max}
            color={colorOf(r)}
            link={links.get(r.id)}
            onOpen={onOpen}
            onReclaim={onReclaim}
          />
        ))}
        {s.filesHere !== null && (
          <div className="hairline-b flex items-center gap-3.5 py-2 last:border-0">
            <div
              className="h-[22px] w-2 flex-none rounded-[3px]"
              style={{ background: "var(--track)" }}
            />
            <div className="w-[190px] flex-none truncate text-[12.5px]" style={{ color: "var(--txt2)" }}>
              Files directly in {s.title}
            </div>
            <div
              className="h-[9px] min-w-0 flex-1 overflow-hidden rounded-[5px]"
              style={{ background: "var(--track)" }}
            >
              <div
                className="h-full rounded-[5px]"
                style={{ width: `${(s.filesHere / max) * 100}%`, background: "var(--c7)" }}
              />
            </div>
            <div
              className="mono w-[86px] flex-none text-right text-[12px] font-semibold"
              style={{ color: "var(--txt2)" }}
            >
              {fmtBytes(s.filesHere)}
            </div>
            <div className="w-6 flex-none" />
          </div>
        )}
        {s.other && (
          <div className="mono py-2 text-[10.5px]" style={{ color: "var(--txt3)" }}>
            + {s.other.count.toLocaleString()} smaller · {fmtBytes(s.other.bytes)}
          </div>
        )}
        {s.rows.length === 0 && s.filesHere === null && (
          <div className="py-6 text-center text-[12px]" style={{ color: "var(--txt3)" }}>
            {s.loading || s.measuringHere ? "Looking inside…" : "Measuring starts in a moment."}
          </div>
        )}
      </div>

      {notice && top && (
        <div
          className="mb-2.5 mt-1.5 flex items-center gap-3 rounded-[13px] px-3.5 py-2.5"
          style={{
            background: "color-mix(in srgb, var(--warn) 10%, transparent)",
            boxShadow: "0 0 0 1px color-mix(in srgb, var(--warn) 24%, transparent)",
          }}
        >
          <span className="min-w-0 flex-1 text-[11.5px] leading-snug" style={{ color: "var(--txt2)" }}>
            {notice}
          </span>
          <button
            onClick={onFullDiskAccess}
            className="glass-chip btn-focus h-7 flex-none rounded-lg px-3 text-[11px] font-semibold"
            style={{ color: "var(--txt)" }}
          >
            Open Privacy settings
          </button>
        </div>
      )}
    </section>
  );
}

function DetailRow({
  row: r,
  max,
  color,
  link,
  onOpen,
  onReclaim,
}: {
  row: Row;
  max: number;
  color: string;
  link: Link | undefined;
  onOpen: (crumb: Crumb) => void;
  onReclaim: (card: Card) => void;
}) {
  const open = () => onOpen({ id: r.id, name: r.name });
  const measuring = r.state !== "done";
  const body = (
    <>
      <div className="h-[22px] w-2 flex-none rounded-[3px]" style={{ background: color }} />
      <div className="w-[190px] flex-none text-left">
        <div className="truncate text-[12.5px] font-medium" style={{ color: "var(--txt)" }}>
          {r.name}
        </div>
        <div className="mono truncate text-[9.5px]" style={{ color: "var(--txt3)" }}>
          {r.refreshing
            ? `updating · ${fmtBytes(r.measured)} so far`
            : measuring
              ? "measuring"
              : `${(r.share * 100).toFixed(r.share < 0.1 ? 1 : 0)}%`}
        </div>
      </div>
      <div
        className="relative h-[9px] min-w-0 flex-1 overflow-hidden rounded-[5px]"
        style={{ background: "var(--track)" }}
      >
        {r.refreshing ? (
          <>
            {/* The last known size, and the walk filling it in again. */}
            <div
              className="absolute inset-y-0 left-0 rounded-[5px]"
              style={{
                width: `${(r.bytes / max) * 100}%`,
                background: `color-mix(in srgb, ${color} 38%, transparent)`,
              }}
            />
            <div
              className="relative h-full rounded-[5px]"
              style={{
                width: `${(r.measured / max) * 100}%`,
                background: color,
                transition: "width .5s ease",
              }}
            />
          </>
        ) : (
          r.bytes > 0 && (
            <div
              className="relative h-full rounded-[5px]"
              style={{
                width: `${(r.bytes / max) * 100}%`,
                background: `linear-gradient(90deg, ${color}, color-mix(in oklch, ${color}, white 22%))`,
                transition: "width .5s ease",
              }}
            />
          )
        )}
      </div>
      {link && (
        <button
          onClick={(e) => {
            e.stopPropagation();
            onReclaim(link.cards[0]);
          }}
          title={link.cards.map((c) => `${c.title} — ${fmtKB(c.size_kb)}`).join("\n")}
          className="btn-focus h-6 flex-none rounded-md px-2 text-[10.5px] font-semibold"
          style={{
            color: "var(--good)",
            background: "color-mix(in srgb, var(--good) 12%, transparent)",
            boxShadow: "0 0 0 1px color-mix(in srgb, var(--good) 26%, transparent)",
          }}
        >
          {fmtKB(link.kb)} reclaimable
        </button>
      )}
      <div className="flex w-[86px] flex-none items-center justify-end gap-1.5">
        {measuring && <Spinner />}
        <span
          className="mono text-right text-[12px] font-semibold"
          style={{ color: measuring ? "var(--txt2)" : "var(--txt)" }}
        >
          {r.bytes > 0 ? fmtBytes(r.bytes) : "—"}
        </span>
      </div>
      <div className="flex w-6 flex-none justify-center" style={{ color: "var(--txt3)" }}>
        {r.openable && <Chevron />}
      </div>
    </>
  );

  // A div, not a button: the reclaim chip inside it is a button of its own.
  return r.openable ? (
    <div
      role="button"
      tabIndex={0}
      onClick={open}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          open();
        }
      }}
      className="hairline-b btn-focus -mx-2 flex cursor-pointer items-center gap-3.5 rounded-lg px-2 py-2 transition-colors last:border-0 hover:bg-(--track)"
      title={`Open ${r.name}`}
    >
      {body}
    </div>
  ) : (
    <div className="hairline-b flex items-center gap-3.5 py-2 last:border-0">{body}</div>
  );
}
