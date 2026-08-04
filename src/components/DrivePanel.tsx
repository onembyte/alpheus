import { useState } from "react";
import type { useDrive } from "../hooks/useDrive";
import { fmtBytes } from "../format";

interface Props {
  d: ReturnType<typeof useDrive>;
  onOpenSettings: () => void;
}

interface Confirm {
  title: string;
  detail: string;
  label: string;
  danger: boolean;
  run: () => void;
}

export default function DrivePanel({ d, onOpenSettings }: Props) {
  const [confirm, setConfirm] = useState<Confirm | null>(null);

  if (d.connected === false) {
    return (
      <div className="glass-card fade-up mx-auto max-w-[560px] px-[18px] py-12 text-center">
        <div className="text-[14px] font-semibold" style={{ color: "var(--txt)" }}>
          Google Drive isn't connected yet
        </div>
        <div className="mx-auto mt-2 max-w-[420px] text-[11.5px] leading-relaxed" style={{ color: "var(--txt3)" }}>
          Connect your own Google OAuth client in Settings — Alpheus then shows your
          quota, finds exact duplicates by checksum, and reclaims space into Drive's
          own 30-day Trash.
        </div>
        <button
          onClick={onOpenSettings}
          className="glass-chip btn-focus mt-4 h-8 rounded-lg px-4 text-[12px] font-semibold"
          style={{ color: "var(--txt)" }}
        >
          Open Settings
        </button>
      </div>
    );
  }

  const q = d.quota;
  const otherBytes = q ? Math.max(0, q.usageBytes - q.driveBytes - q.trashBytes) : 0;
  const freeBytes = q ? Math.max(0, q.limitBytes - q.usageBytes) : 0;
  const dupeTotal = (d.dupes ?? []).reduce((s, x) => s + x.reclaimBytes, 0);

  return (
    <div className="fade-up flex flex-col gap-4">
      {/* quota */}
      <div className="glass-card px-[18px] pb-[18px] pt-4">
        <div className="mb-3 flex items-baseline justify-between gap-3">
          <div className="section-label">Google Drive</div>
          <div className="mono min-w-0 truncate text-[10px]" style={{ color: "var(--txt3)" }}>
            {q ? `${q.email} · ${fmtBytes(freeBytes)} free of ${fmtBytes(q.limitBytes)}` : "quota loads with the first analysis"}
          </div>
        </div>

        {q && (
          <>
            <div
              className="relative flex h-[26px] gap-[2px] overflow-hidden rounded-[9px]"
              style={{ background: "var(--track)", boxShadow: "inset 0 1px 3px rgba(0,0,0,.22)" }}
            >
              {[
                { kb: q.driveBytes, color: "var(--c1)", label: "Drive files" },
                { kb: q.trashBytes, color: "var(--c4)", label: "Drive Trash" },
                { kb: otherBytes, color: "var(--c7)", label: "Gmail & Photos" },
              ].map(
                (s) =>
                  s.kb > 0 &&
                  q.limitBytes > 0 && (
                    <div
                      key={s.label}
                      className="bar-in"
                      title={`${s.label} — ${fmtBytes(s.kb)}`}
                      style={{ width: `${(s.kb / q.limitBytes) * 100}%`, background: s.color }}
                    />
                  ),
              )}
              <div className="flex-1" />
              <div
                className="pointer-events-none absolute inset-0"
                style={{
                  background:
                    "linear-gradient(180deg, rgba(255,255,255,.42), rgba(255,255,255,.06) 48%, rgba(0,0,0,.10))",
                }}
              />
            </div>
            <div className="mono mt-2 flex gap-4 text-[10.5px]" style={{ color: "var(--txt3)" }}>
              <span>drive {fmtBytes(q.driveBytes)}</span>
              <span style={{ color: "var(--warn)" }}>trash {fmtBytes(q.trashBytes)}</span>
              <span>gmail+photos {fmtBytes(otherBytes)}</span>
            </div>
          </>
        )}

        <div className="mt-3.5 flex items-center gap-3">
          <button
            onClick={d.analyze}
            disabled={d.analyzing || d.busy}
            className="btn-focus relative flex h-[30px] items-center gap-[7px] overflow-hidden rounded-[9px] px-[15px] text-[12px] font-semibold disabled:opacity-70"
            style={{
              background:
                "linear-gradient(180deg, color-mix(in srgb, var(--accent) 55%, transparent), color-mix(in srgb, var(--accent) 28%, transparent))",
              boxShadow:
                "inset 0 1px 0 rgba(255,255,255,.6), 0 0 0 1px color-mix(in srgb, var(--accent) 40%, transparent)",
              color: "#fff",
              textShadow: "0 1px 2px rgba(0,0,0,.25)",
            }}
          >
            {!d.analyzing && <div className="sheen" />}
            <span className="relative">{d.analyzing ? "Analyzing…" : "Analyze Drive"}</span>
          </button>
          {d.analyzing && (
            <span className="mono text-[10.5px]" style={{ color: "var(--txt3)" }}>
              {d.progress.toLocaleString()} files listed — checksums come free, nothing downloads
            </span>
          )}
          {q && q.trashBytes > 0 && (
            <button
              onClick={() =>
                setConfirm({
                  title: "Empty Drive Trash?",
                  detail: `${fmtBytes(q.trashBytes)} is deleted permanently from Google's servers. This is the one Drive action that can't be undone.`,
                  label: "Empty Trash",
                  danger: true,
                  run: d.emptyDriveTrash,
                })
              }
              disabled={d.busy}
              className="glass-chip btn-focus ml-auto h-[30px] rounded-[9px] px-3.5 text-[11.5px] font-semibold disabled:opacity-50"
              style={{ color: "var(--warn)" }}
            >
              Empty Drive Trash · {fmtBytes(q.trashBytes)}
            </button>
          )}
        </div>
      </div>

      {/* duplicates */}
      {d.dupes && (
        <div className="glass-card px-[18px] pb-1 pt-4">
          <div className="mb-1.5 flex items-baseline justify-between gap-3">
            <div className="section-label">
              Duplicates · {d.dupes.length} set{d.dupes.length === 1 ? "" : "s"}
            </div>
            <div className="mono text-[10.5px]" style={{ color: "var(--good)" }}>
              {fmtBytes(dupeTotal)} reclaimable
            </div>
          </div>
          {d.dupes.length === 0 && (
            <div className="py-8 text-center text-[12px]" style={{ color: "var(--txt3)" }}>
              No exact duplicates among your files — clean Drive.
            </div>
          )}
          {d.dupes.slice(0, 25).map((s) => (
            <div key={s.key} className="hairline-b flex items-center gap-3.5 py-2.5 last:border-0">
              <div className="min-w-0 flex-1">
                <div className="truncate text-[12.5px] font-medium" style={{ color: "var(--txt)" }}>
                  {s.name}
                </div>
                <div className="mono truncate text-[9.5px]" style={{ color: "var(--txt3)" }}>
                  keeps newest · trashes {s.redundant.length} cop{s.redundant.length === 1 ? "y" : "ies"}
                </div>
              </div>
              <span className="mono w-[52px] flex-none text-right text-[11px]" style={{ color: "var(--txt2)" }}>
                ×{s.copies}
              </span>
              <span className="mono w-[70px] flex-none text-right text-[11px]" style={{ color: "var(--txt2)" }}>
                {fmtBytes(s.eachBytes)}
              </span>
              <span className="mono w-[76px] flex-none text-right text-[12px] font-semibold" style={{ color: "var(--good)" }}>
                {fmtBytes(s.reclaimBytes)}
              </span>
              <button
                onClick={() =>
                  setConfirm({
                    title: `Trash ${s.redundant.length} redundant cop${s.redundant.length === 1 ? "y" : "ies"}?`,
                    detail: `"${s.name}" — the newest copy stays; ${fmtBytes(s.reclaimBytes)} moves to Drive's Trash (recoverable for 30 days).`,
                    label: "Move to Drive Trash",
                    danger: false,
                    run: () => d.trashDuplicates(s),
                  })
                }
                disabled={d.busy}
                className="glass-chip btn-focus h-7 flex-none rounded-lg px-3 text-[11.5px] font-semibold disabled:opacity-50"
                style={{ color: "var(--txt)" }}
              >
                Trash copies
              </button>
            </div>
          ))}
        </div>
      )}

      {/* largest files */}
      {d.largest && d.largest.length > 0 && (
        <div className="glass-card px-[18px] pb-1 pt-4">
          <div className="section-label mb-1.5">Largest files</div>
          {d.largest.map((f) => (
            <div key={f.id} className="hairline-b flex items-center gap-3.5 py-2 last:border-0">
              <div className="min-w-0 flex-1">
                <div className="truncate text-[12.5px]" style={{ color: "var(--txt)" }}>
                  {f.name}
                </div>
              </div>
              <span className="mono w-[80px] flex-none text-right text-[12px] font-semibold" style={{ color: "var(--txt)" }}>
                {fmtBytes(f.sizeBytes)}
              </span>
              <button
                onClick={() =>
                  setConfirm({
                    title: `Trash "${f.name}"?`,
                    detail: `${fmtBytes(f.sizeBytes)} moves to Drive's Trash (recoverable for 30 days).`,
                    label: "Move to Drive Trash",
                    danger: false,
                    run: () => d.trashSingle(f),
                  })
                }
                disabled={d.busy}
                className="glass-chip btn-focus h-7 flex-none rounded-lg px-3 text-[11.5px] font-semibold disabled:opacity-50"
                style={{ color: "var(--txt)" }}
              >
                Trash
              </button>
            </div>
          ))}
        </div>
      )}

      {/* lightweight confirm */}
      {confirm && (
        <div
          className="absolute inset-0 z-50 flex items-center justify-center p-6"
          style={{ background: "rgba(0,0,0,.34)", backdropFilter: "blur(3px)" }}
          onClick={() => setConfirm(null)}
          role="dialog"
          aria-modal="true"
        >
          <div
            onClick={(e) => e.stopPropagation()}
            className="modal-pop w-[420px] rounded-[20px] p-6 pb-5"
            style={{
              background: "linear-gradient(170deg, var(--glass-a), var(--glass-b)), var(--win)",
              backdropFilter: "blur(50px) saturate(200%)",
              boxShadow:
                "0 40px 90px -20px rgba(0,0,0,.7), inset 0 -1px 0 var(--edge-lo), 0 0 0 1px var(--edge)",
            }}
          >
            <div className="text-[15px] font-semibold" style={{ color: "var(--txt)" }}>
              {confirm.title}
            </div>
            <div className="mt-1.5 text-[11.5px] leading-relaxed" style={{ color: "var(--txt2)" }}>
              {confirm.detail}
            </div>
            <div className="mt-4 flex justify-end gap-2.5">
              <button
                onClick={() => setConfirm(null)}
                className="glass-chip btn-focus flex h-[31px] items-center rounded-[9px] px-[17px] text-[12px] font-medium"
                style={{ color: "var(--txt)" }}
              >
                Cancel
              </button>
              <button
                onClick={() => {
                  confirm.run();
                  setConfirm(null);
                }}
                className="btn-focus flex h-[31px] items-center rounded-[9px] px-[17px] text-[12px] font-semibold"
                style={{
                  background: `linear-gradient(180deg, color-mix(in srgb, ${confirm.danger ? "var(--danger)" : "var(--good)"} 62%, transparent), color-mix(in srgb, ${confirm.danger ? "var(--danger)" : "var(--good)"} 31%, transparent))`,
                  boxShadow: `inset 0 1px 0 rgba(255,255,255,.5), 0 0 0 1px color-mix(in srgb, ${confirm.danger ? "var(--danger)" : "var(--good)"} 45%, transparent)`,
                  color: confirm.danger ? "#fff" : "var(--on-good)",
                }}
              >
                {confirm.label}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
