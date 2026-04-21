import { useEffect, useMemo, useState } from "react";
import { api } from "../api/tauri";
import { useStore, type FcPolicy } from "../store";
import RouteRow from "../components/RouteRow";
import RouteDetail from "../components/RouteDetail";
import type { RankedRoute, RouteFilter, RouteMode } from "../types";

const ALL_MODES: RouteMode[] = [
  "single",
  "loop2",
  "loop3",
  "loop4",
  "rare_chain",
];

export default function LiveRoutes() {
  const routes = useStore((s) => s.routes);
  const userState = useStore((s) => s.userState);
  const routeFilter = useStore((s) => s.routeFilter);
  const [selected, setSelected] = useState<RankedRoute | null>(null);

  const { maxJumps, minCrHr, maxPpt, modes, fcPolicy } = routeFilter;
  const setFilter = (patch: Partial<typeof routeFilter>) =>
    useStore.setState((s) => ({ routeFilter: { ...s.routeFilter, ...patch } }));
  const modeSet = useMemo(() => new Set(modes), [modes]);

  useEffect(() => {
    const filter: RouteFilter = {
      modes,
      max_jumps: maxJumps,
      min_cr_per_hour: BigInt(minCrHr),
      max_profit_per_ton: maxPpt > 0 ? maxPpt : null,
      pad_size_min: null,
      allow_anarchy: true,
      require_fleet_carrier: fcPolicy === "require",
      exclude_fleet_carrier: fcPolicy === "exclude",
      limit: 50,
    };
    api
      .getTopRoutes(filter)
      .then((r) => useStore.setState({ routes: r }))
      .catch(() => {});
  }, [maxJumps, minCrHr, modes, fcPolicy, maxPpt]);

  const filtered = useMemo(
    () =>
      routes.filter(
        (r) =>
          modeSet.has(r.mode) &&
          r.total_jumps <= maxJumps &&
          Number(r.cr_per_hour) >= minCrHr &&
          (fcPolicy !== "exclude" || !r.touches_fleet_carrier) &&
          (fcPolicy !== "require" || r.touches_fleet_carrier) &&
          (maxPpt <= 0 || r.legs.every((l) => l.profit_per_ton <= maxPpt)),
      ),
    [routes, modeSet, maxJumps, minCrHr, fcPolicy, maxPpt],
  );

  return (
    <div className="p-4">
      <div className="flex gap-4 items-center mb-4 text-sm flex-wrap">
        <div>
          <label className="text-[var(--color-text-dim)] mr-2">Max jumps</label>
          <input
            type="number"
            value={maxJumps}
            onChange={(e) => setFilter({ maxJumps: Number(e.target.value) })}
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1 w-16 tabular-nums"
          />
        </div>
        <div>
          <label className="text-[var(--color-text-dim)] mr-2">Min cr/hr</label>
          <input
            type="number"
            value={minCrHr}
            onChange={(e) => setFilter({ minCrHr: Number(e.target.value) })}
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1 w-32 tabular-nums"
          />
        </div>
        <div title="Hide legs with profit/ton above this — filters FC joke prices. 0 disables.">
          <label className="text-[var(--color-text-dim)] mr-2">Max cr/ton</label>
          <input
            type="number"
            value={maxPpt}
            onChange={(e) => setFilter({ maxPpt: Number(e.target.value) })}
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1 w-28 tabular-nums"
          />
        </div>
        <div className="flex gap-2">
          {ALL_MODES.map((m) => (
            <button
              type="button"
              key={m}
              onClick={() => {
                const next = modeSet.has(m)
                  ? modes.filter((x) => x !== m)
                  : [...modes, m];
                setFilter({ modes: next });
              }}
              className={
                modeSet.has(m)
                  ? "px-2 py-1 border border-[var(--color-accent)] text-[var(--color-accent)] text-xs"
                  : "px-2 py-1 border border-[var(--color-border)] text-[var(--color-text-dim)] text-xs"
              }
            >
              {m}
            </button>
          ))}
        </div>
        <div
          className="flex gap-1"
          title="Fleet carrier policy: any / exclude FC routes / only FC routes"
        >
          {(["any", "exclude", "require"] as const).map((p) => (
            <button
              type="button"
              key={p}
              onClick={() => setFilter({ fcPolicy: p as FcPolicy })}
              className={
                fcPolicy === p
                  ? "px-2 py-1 border border-[var(--color-accent)] text-[var(--color-accent)] text-xs"
                  : "px-2 py-1 border border-[var(--color-border)] text-[var(--color-text-dim)] text-xs"
              }
            >
              {p === "any" ? "FC: any" : p === "exclude" ? "FC: hide" : "FC: only"}
            </button>
          ))}
        </div>
        <div className="flex-1 text-right text-xs text-[var(--color-text-dim)]">
          {userState?.current_system
            ? `at ${userState.current_system}`
            : "no journal state"}
        </div>
      </div>

      <table className="w-full">
        <thead>
          <tr className="text-[var(--color-text-dim)] text-xs uppercase border-b border-[var(--color-border)]">
            <th
              className="px-3 py-2 text-left"
              title="Single-hop, 2-leg loop, 3-4 leg loop, or rare-goods chain"
            >
              Mode
            </th>
            <th
              className="px-3 py-2 text-left"
              title="Station (System) → Station (System). Click row for copyable details."
            >
              Path
            </th>
            <th
              className="px-3 py-2 text-right"
              title="Estimated credits per hour assuming a sustainable cycle, factoring jumps + docking + supercruise time"
            >
              Cr/hr
            </th>
            <th
              className="px-3 py-2 text-right"
              title="Estimated time to complete one full cycle (jumps + supercruise + docking + market)"
            >
              Cycle
            </th>
            <th
              className="px-3 py-2 text-right"
              title="Total profit for one full cycle, bounded by min(supply, demand, your cargo capacity)"
            >
              Profit/cycle
            </th>
            <th
              className="px-3 py-2 text-left"
              title="Sustainable: demand ≥ 10× cargo, you can repeat the cycle indefinitely. Decaying N cycles: drains to zero after roughly N runs."
            >
              Sustain
            </th>
            <th
              className="px-3 py-2"
              title="Freshness of the most-stale market snapshot in this route. Green < 2m, yellow < 10m, gray older."
            >
              Fresh
            </th>
          </tr>
        </thead>
        <tbody>
          {filtered.map((r) => (
            <RouteRow key={r.route_hash} r={r} onClick={() => setSelected(r)} />
          ))}
          {filtered.length === 0 && (
            <tr>
              <td
                colSpan={7}
                className="text-center py-8 text-[var(--color-text-dim)]"
              >
                No routes yet — waiting for EDDN data
              </td>
            </tr>
          )}
        </tbody>
      </table>

      {selected && (
        <RouteDetail route={selected} onClose={() => setSelected(null)} />
      )}
    </div>
  );
}
