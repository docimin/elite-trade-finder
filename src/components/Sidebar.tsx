import clsx from "clsx";
import pkg from "../../package.json";

export type View = "routes" | "firehose" | "commander" | "settings";

const ITEMS: Array<{ key: View; label: string; hint: string }> = [
  { key: "routes", label: "Live Routes", hint: "Top routes by cr/hr" },
  { key: "firehose", label: "Firehose", hint: "EDDN stream + alerts" },
  { key: "commander", label: "Commander", hint: "Your ship + state" },
  { key: "settings", label: "Settings", hint: "Config + diagnostics" },
];

export default function Sidebar({
  active,
  onSelect,
}: {
  active: View;
  onSelect: (v: View) => void;
}) {
  return (
    <nav className="w-56 border-r border-[var(--color-border)] bg-[var(--color-panel)] flex flex-col">
      <div className="px-4 py-4 border-b border-[var(--color-border)]">
        <div className="text-[var(--color-accent)] font-semibold tracking-wide">
          Elite Trade Finder
        </div>
        <div className="text-xs text-[var(--color-text-dim)]">
          v{pkg.version}
          {import.meta.env.DEV ? " dev" : ""}
        </div>
      </div>
      <ul className="flex-1 py-2">
        {ITEMS.map((it) => (
          <li key={it.key}>
            <button
              onClick={() => onSelect(it.key)}
              className={clsx(
                "w-full text-left px-4 py-3 hover:bg-[var(--color-panel-hi)] border-l-2",
                active === it.key
                  ? "border-[var(--color-accent)] bg-[var(--color-panel-hi)]"
                  : "border-transparent",
              )}
            >
              <div className="text-sm text-[var(--color-text)]">{it.label}</div>
              <div className="text-xs text-[var(--color-text-dim)]">
                {it.hint}
              </div>
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
