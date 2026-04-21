import { useEffect, useState } from "react";
import type { RankedRoute } from "../types";

function Copyable({ value, label }: { value: string; label?: string }) {
  const [copied, setCopied] = useState(false);
  async function doCopy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      /* ignore */
    }
  }
  return (
    <span
      onClick={doCopy}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          doCopy();
        }
      }}
      className="font-mono hover:bg-[var(--color-panel-hi)] px-1 py-0.5 rounded inline-flex items-center gap-2 cursor-pointer select-text"
      style={{ userSelect: "text", WebkitUserSelect: "text" } as React.CSSProperties}
      title="Click to copy (or drag-select + Ctrl+C)"
    >
      <span>{value}</span>
      {label && (
        <span className="text-xs text-[var(--color-text-dim)]">{label}</span>
      )}
      {copied && (
        <span className="text-xs text-[var(--color-good)]">copied</span>
      )}
    </span>
  );
}

export default function RouteDetail({
  route,
  onClose,
}: {
  route: RankedRoute;
  onClose: () => void;
}) {
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const crHr = (Number(route.cr_per_hour) / 1_000_000).toFixed(2);
  const sustain =
    route.sustainability === "sustainable"
      ? "Sustainable (demand ≥ 10× cargo)"
      : `Decaying — good for ~${(route.sustainability as any).decaying?.estimated_cycles ?? "?"} cycle(s)`;

  return (
    <div
      className="fixed inset-0 bg-black/60 flex items-center justify-center z-50"
      onClick={onClose}
    >
      <div
        className="bg-[var(--color-panel)] border border-[var(--color-border)] max-w-3xl w-full max-h-[85vh] overflow-auto p-6"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between mb-4">
          <div>
            <div className="text-[var(--color-accent)] font-semibold">
              {crHr}M cr/hr · {route.mode}
            </div>
            <div className="text-xs text-[var(--color-text-dim)]">
              {route.profit_per_cycle.toLocaleString()} cr / cycle ·{" "}
              {Math.round(route.cycle_seconds / 60)} min · {route.total_jumps} jumps
            </div>
            <div className="text-xs text-[var(--color-text-dim)] mt-1">{sustain}</div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="px-2 py-1 border border-[var(--color-border)] text-xs"
          >
            ✕ close (Esc)
          </button>
        </div>

        <div className="text-xs text-[var(--color-text-dim)] uppercase mb-2">
          Legs · click any value to copy
        </div>
        <ol className="flex flex-col gap-3">
          {route.legs.map((l, i) => (
            <li
              key={i}
              className="border border-[var(--color-border)] p-3 text-sm"
            >
              <div className="text-xs text-[var(--color-text-dim)] mb-2">
                Leg {i + 1} · {l.profit_per_ton.toLocaleString()} cr/ton ·{" "}
                {l.jumps} jumps · {l.distance_ly.toFixed(1)} ly
              </div>
              <div className="grid grid-cols-[80px_1fr] gap-y-1 gap-x-3 items-center">
                <span className="text-[var(--color-text-dim)] text-xs">
                  From system
                </span>
                <Copyable value={l.from_system} />
                <span className="text-[var(--color-text-dim)] text-xs">
                  From station
                </span>
                <Copyable value={l.from_station} />
                <span className="text-[var(--color-text-dim)] text-xs">
                  Commodity
                </span>
                <Copyable value={l.commodity} />
                <span className="text-[var(--color-text-dim)] text-xs">
                  To system
                </span>
                <Copyable value={l.to_system} />
                <span className="text-[var(--color-text-dim)] text-xs">
                  To station
                </span>
                <Copyable value={l.to_station} />
                <span className="text-[var(--color-text-dim)] text-xs">
                  Prices
                </span>
                <span className="font-mono text-xs">
                  buy {l.buy_price.toLocaleString()} · sell{" "}
                  {l.sell_price.toLocaleString()} · supply {l.supply} ·
                  demand {l.demand}
                </span>
              </div>
            </li>
          ))}
        </ol>
      </div>
    </div>
  );
}
