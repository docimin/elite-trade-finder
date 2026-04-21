import clsx from "clsx";
import type { RankedRoute } from "../types";

function modeLabel(m: RankedRoute["mode"]) {
  switch (m) {
    case "single":
      return "SINGLE";
    case "loop2":
      return "LOOP-2";
    case "loop3":
      return "LOOP-3";
    case "loop4":
      return "LOOP-4";
    case "rare_chain":
      return "RARE";
  }
}

function freshness(seconds: number) {
  if (seconds < 120) return "fresh";
  if (seconds < 600) return "stale";
  return "dead";
}

export default function RouteRow({
  r,
  onClick,
}: {
  r: RankedRoute;
  onClick: () => void;
}) {
  const f = freshness(r.freshest_age_seconds);
  const stops = r.legs
    .map((l) => ({ system: l.from_system, station: l.from_station }))
    .concat([
      {
        system: r.legs[r.legs.length - 1].to_system,
        station: r.legs[r.legs.length - 1].to_station,
      },
    ]);
  const cycleMin = (r.cycle_seconds / 60).toFixed(1);
  const crHr = (Number(r.cr_per_hour) / 1_000_000).toFixed(2);
  const sustain =
    r.sustainability === "sustainable"
      ? "sustainable"
      : `${(r.sustainability as any).decaying?.estimated_cycles ?? "?"} cycles`;

  return (
    <tr
      onClick={onClick}
      className="hover:bg-[var(--color-panel-hi)] cursor-pointer border-b border-[var(--color-border)]"
    >
      <td className="px-3 py-2 text-[var(--color-accent)] font-mono text-xs">
        {modeLabel(r.mode)}
      </td>
      <td className="px-3 py-2 text-sm">
        {stops.map((s, i) => (
          <span key={i}>
            {i > 0 && <span className="text-[var(--color-text-dim)] mx-1">→</span>}
            <span className="font-mono">{s.station}</span>
            <span className="text-[var(--color-text-dim)] text-xs ml-1">
              ({s.system})
            </span>
          </span>
        ))}
      </td>
      <td className="px-3 py-2 text-right tabular-nums font-mono text-[var(--color-good)]">
        {crHr}M
      </td>
      <td className="px-3 py-2 text-right tabular-nums font-mono text-[var(--color-text-dim)]">
        {cycleMin}m
      </td>
      <td className="px-3 py-2 text-right tabular-nums font-mono text-[var(--color-text-dim)]">
        {r.profit_per_cycle.toLocaleString()}
      </td>
      <td className="px-3 py-2 text-xs text-[var(--color-text-dim)]">{sustain}</td>
      <td className="px-3 py-2">
        <span
          className={clsx(
            "inline-block w-2 h-2 rounded-full",
            f === "fresh" && "bg-[var(--color-fresh)]",
            f === "stale" && "bg-[var(--color-stale)]",
            f === "dead" && "bg-[var(--color-dead)]",
          )}
        />
      </td>
    </tr>
  );
}
