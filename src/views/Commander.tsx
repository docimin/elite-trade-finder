import { useEffect } from "react";
import { api } from "../api/tauri";
import { useStore } from "../store";
import type { ShipSpec } from "../types";

const DEFAULT_OVERRIDE: ShipSpec = {
  ship_type: "custom",
  cargo_capacity: 0,
  jump_range_ly: 0,
  pad_size_max: "L",
};

export default function Commander() {
  const userState = useStore((s) => s.userState);
  const refreshUserState = useStore((s) => s.refreshUserState);
  const override = useStore((s) => s.commanderOverride);

  useEffect(() => {
    const id = setInterval(refreshUserState, 5000);
    return () => clearInterval(id);
  }, [refreshUserState]);

  function updateOverride(patch: Partial<ShipSpec>) {
    const base = override ?? DEFAULT_OVERRIDE;
    useStore.setState({ commanderOverride: { ...base, ...patch } });
  }

  async function applyOverride(ship: ShipSpec | null) {
    await api.manualOverrideShip(ship);
    useStore.setState({ commanderOverride: ship });
  }

  const current = override ?? DEFAULT_OVERRIDE;

  return (
    <div className="p-6 grid gap-6">
      <section>
        <h2 className="text-lg mb-3">Live state (from journal)</h2>
        <dl className="grid grid-cols-[160px_1fr] gap-y-2 text-sm">
          <dt className="text-[var(--color-text-dim)]">System</dt>
          <dd className="font-mono">{userState?.current_system ?? "—"}</dd>
          <dt className="text-[var(--color-text-dim)]">Station</dt>
          <dd className="font-mono">{userState?.current_station ?? "—"}</dd>
          <dt className="text-[var(--color-text-dim)]">Ship</dt>
          <dd className="font-mono">{userState?.ship_type ?? "—"}</dd>
          <dt className="text-[var(--color-text-dim)]">Cargo cap</dt>
          <dd className="font-mono">{userState?.cargo_capacity ?? "—"} t</dd>
          <dt className="text-[var(--color-text-dim)]">Jump range</dt>
          <dd className="font-mono">
            {userState?.jump_range_ly?.toFixed(1) ?? "—"} ly
          </dd>
          <dt className="text-[var(--color-text-dim)]">Credits</dt>
          <dd className="font-mono">
            {userState?.credits?.toLocaleString() ?? "—"} cr
          </dd>
          <dt className="text-[var(--color-text-dim)]">Max pad</dt>
          <dd className="font-mono">{userState?.pad_size_max ?? "—"}</dd>
        </dl>
      </section>

      <section>
        <h2 className="text-lg mb-3">What-if override</h2>
        <p className="text-xs text-[var(--color-text-dim)] mb-3">
          Temporarily pretend you're in a different ship — routes will recompute
          for that spec until cleared.
        </p>
        <div className="grid grid-cols-[120px_1fr] gap-3 text-sm max-w-xl">
          <label className="text-[var(--color-text-dim)]">Ship type</label>
          <input
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1"
            value={current.ship_type}
            onChange={(e) => updateOverride({ ship_type: e.target.value })}
          />
          <label className="text-[var(--color-text-dim)]">Cargo cap</label>
          <input
            type="number"
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1"
            value={current.cargo_capacity}
            onChange={(e) =>
              updateOverride({ cargo_capacity: Number(e.target.value) })
            }
          />
          <label className="text-[var(--color-text-dim)]">Jump range (ly)</label>
          <input
            type="number"
            step="0.1"
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1"
            value={current.jump_range_ly}
            onChange={(e) =>
              updateOverride({ jump_range_ly: Number(e.target.value) })
            }
          />
          <label className="text-[var(--color-text-dim)]">Max pad</label>
          <select
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1"
            value={current.pad_size_max}
            onChange={(e) => updateOverride({ pad_size_max: e.target.value })}
          >
            <option>S</option>
            <option>M</option>
            <option>L</option>
          </select>
        </div>
        <div className="flex gap-3 mt-3">
          <button
            type="button"
            className="px-3 py-1 border border-[var(--color-accent)] text-[var(--color-accent)] text-sm disabled:opacity-50"
            onClick={() => applyOverride(override)}
            disabled={!override}
          >
            Apply
          </button>
          <button
            type="button"
            className="px-3 py-1 border border-[var(--color-border)] text-sm"
            onClick={() => applyOverride(null)}
          >
            Clear
          </button>
        </div>
      </section>
    </div>
  );
}
