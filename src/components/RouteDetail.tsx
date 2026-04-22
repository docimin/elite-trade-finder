import { useEffect, useState } from "react";
import type { RankedRoute } from "../types";
import { useStore } from "../store";

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

function formatAge(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.round(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.round(seconds / 3600)}h`;
  return `${Math.round(seconds / 86400)}d`;
}

function modeLabel(m: RankedRoute["mode"]): string {
  switch (m) {
    case "single": return "Single hop";
    case "loop2": return "2-leg loop";
    case "loop3": return "3-leg loop";
    case "loop4": return "4-leg loop";
    case "rare_chain": return "Rare chain";
  }
}

export default function RouteDetail({
  route,
  onClose,
}: {
  route: RankedRoute;
  onClose: () => void;
}) {
  const userState = useStore((s) => s.userState);
  const override = useStore((s) => s.commanderOverride);

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

  const totalDistanceLy = route.legs.reduce((a, l) => a + l.distance_ly, 0);
  const cargoCap =
    override?.cargo_capacity ?? userState?.cargo_capacity ?? 0;
  // Per-leg "units this run" = min(supply, demand, cargo capacity)
  const legUnits = route.legs.map((l) =>
    Math.min(l.supply, l.demand, cargoCap > 0 ? cargoCap : l.supply),
  );
  const legProfits = route.legs.map(
    (l, i) => l.profit_per_ton * legUnits[i],
  );

  return (
    <div
      className="fixed inset-0 bg-black/60 flex items-center justify-center z-50"
      onClick={onClose}
    >
      <div
        className="bg-[var(--color-panel)] border border-[var(--color-border)] max-w-4xl w-full max-h-[90vh] overflow-auto p-6"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between mb-4">
          <div>
            <div className="text-[var(--color-accent)] font-semibold text-lg">
              {crHr}M cr/hr · {modeLabel(route.mode)}
              {route.touches_fleet_carrier && (
                <span
                  className="ml-2 text-xs px-1.5 py-0.5 border border-[var(--color-warn)] text-[var(--color-warn)]"
                  title="Touches a fleet carrier — buy/sell availability depends on the FC owner's permission settings"
                >
                  FC
                </span>
              )}
            </div>
            <div className="text-xs text-[var(--color-text-dim)]">
              {route.profit_per_cycle.toLocaleString()} cr / cycle ·{" "}
              {Math.round(route.cycle_seconds / 60)} min · {route.total_jumps}{" "}
              jumps · {totalDistanceLy.toFixed(1)} ly total
            </div>
            <div className="text-xs text-[var(--color-text-dim)] mt-1">
              {sustain} · freshest leg {formatAge(route.freshest_age_seconds)} ago
            </div>
            <div className="text-xs text-[var(--color-text-dim)] mt-1">
              Score {route.score.toExponential(2)} · cargo assumed:{" "}
              {cargoCap > 0 ? `${cargoCap}t` : "—"}
              {override && (
                <span className="text-[var(--color-accent)] ml-1">(override)</span>
              )}
            </div>
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
          {route.legs.map((l, i) => {
            const units = legUnits[i];
            const legTotal = legProfits[i];
            const ageSec = Math.max(
              0,
              Math.round(
                (Date.now() - new Date(l.recorded_at).getTime()) / 1000,
              ),
            );
            return (
              <li
                key={i}
                className="border border-[var(--color-border)] p-3 text-sm"
              >
                <div className="flex flex-wrap justify-between items-center gap-2 mb-2 text-xs">
                  <div className="text-[var(--color-text-dim)]">
                    <span className="text-[var(--color-text)] font-semibold">
                      Leg {i + 1}
                    </span>{" "}
                    · {l.profit_per_ton.toLocaleString()} cr/ton · {l.jumps}{" "}
                    jumps · {l.distance_ly.toFixed(1)} ly
                  </div>
                  <div className="text-[var(--color-text-dim)]">
                    {units} units × {l.profit_per_ton.toLocaleString()} ={" "}
                    <span className="text-[var(--color-good)] font-mono">
                      {legTotal.toLocaleString()} cr
                    </span>{" "}
                    · data {formatAge(ageSec)} ago
                  </div>
                </div>
                <div className="grid grid-cols-[90px_1fr] gap-y-1 gap-x-3 items-center">
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
                  <Copyable value={l.commodity} label="(buy this)" />
                  <span className="text-[var(--color-text-dim)] text-xs">
                    To system
                  </span>
                  <Copyable value={l.to_system} />
                  <span className="text-[var(--color-text-dim)] text-xs">
                    To station
                  </span>
                  <Copyable value={l.to_station} />
                  <span className="text-[var(--color-text-dim)] text-xs">
                    Buy / Sell
                  </span>
                  <span className="font-mono text-xs">
                    {l.buy_price.toLocaleString()} cr →{" "}
                    <span className="text-[var(--color-good)]">
                      {l.sell_price.toLocaleString()} cr
                    </span>{" "}
                    <span className="text-[var(--color-text-dim)]">
                      (+{l.profit_per_ton.toLocaleString()} cr/ton)
                    </span>
                  </span>
                  <span className="text-[var(--color-text-dim)] text-xs">
                    Market depth
                  </span>
                  <span className="font-mono text-xs">
                    supply {l.supply.toLocaleString()} · demand{" "}
                    {l.demand.toLocaleString()}
                    {l.supply < cargoCap && cargoCap > 0 && (
                      <span className="text-[var(--color-warn)] ml-2">
                        (supply limits you to {l.supply}t, not full cargo)
                      </span>
                    )}
                    {l.demand < cargoCap && cargoCap > 0 && l.demand < l.supply && (
                      <span className="text-[var(--color-warn)] ml-2">
                        (demand limits you to {l.demand}t)
                      </span>
                    )}
                  </span>
                </div>
              </li>
            );
          })}
        </ol>

        <div className="mt-4 text-xs text-[var(--color-text-dim)]">
          Route hash:{" "}
          <span className="font-mono">{route.route_hash}</span>
        </div>
      </div>
    </div>
  );
}
