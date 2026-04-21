import { useEffect, useState } from "react";
import { api } from "../api/tauri";
import { useStore } from "../store";
import type { Settings } from "../types";

function formatBytes(n: number) {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(2)} GB`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)} MB`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)} kB`;
  return `${n} B`;
}

function DiagnoseRoutes() {
  const [report, setReport] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        className="px-3 py-1 border border-[var(--color-border)] text-xs"
        disabled={busy}
        onClick={async () => {
          setBusy(true);
          try {
            const r = await api.debugRoutePipeline();
            setReport(r);
          } catch (e) {
            setReport(String(e));
          } finally {
            setBusy(false);
          }
        }}
      >
        {busy ? "Running…" : "Diagnose route pipeline"}
      </button>
      {report && (
        <div className="max-w-3xl">
          <div className="flex justify-end">
            <button
              type="button"
              className="text-xs px-2 py-1 border border-[var(--color-border)]"
              onClick={async () => {
                try {
                  await navigator.clipboard.writeText(report);
                } catch {
                  /* clipboard may be blocked in some Tauri configs, ignore */
                }
              }}
            >
              Copy
            </button>
          </div>
          <pre className="text-xs font-mono whitespace-pre-wrap bg-[var(--color-panel)] border border-[var(--color-border)] p-3 select-text">
            {report}
          </pre>
        </div>
      )}
    </div>
  );
}

function DatabaseUrlControl({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (v: string | null) => void;
}) {
  const [cleared, setCleared] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const settings = useStore((s) => s.settings);
  const setSettings = useStore((s) => s.setSettings);

  async function useSqlite() {
    console.info("[DatabaseUrlControl] Use SQLite clicked, current value:", value);
    onChange(null);
    setCleared(true);
    setSaveError(null);
    setTimeout(() => setCleared(false), 4000);

    if (!settings) {
      console.warn("[DatabaseUrlControl] settings not loaded, cannot save");
      setSaveError("settings not loaded — click Save settings manually");
      return;
    }
    try {
      await setSettings({
        ...settings,
        data_sources: { ...settings.data_sources, database_url: null },
      });
      console.info("[DatabaseUrlControl] settings saved, database_url=null");
    } catch (e) {
      console.error("[DatabaseUrlControl] save failed", e);
      setSaveError(String(e));
    }
  }
  const [testResult, setTestResult] = useState<{
    state: "idle" | "testing" | "ok" | "fail";
    msg: string;
  }>({ state: "idle", msg: "" });

  async function test() {
    if (!value || !value.trim()) {
      setTestResult({ state: "fail", msg: "URL is empty" });
      return;
    }
    setTestResult({ state: "testing", msg: "connecting…" });
    try {
      const msg = await api.testDatabaseUrl(value);
      setTestResult({ state: "ok", msg });
    } catch (e) {
      setTestResult({
        state: "fail",
        msg: typeof e === "string" ? e : String(e),
      });
    }
  }

  return (
    <>
      <div className="flex gap-2">
        <input
          type="text"
          value={value ?? ""}
          placeholder="sqlite (default) — or postgres://user:pass@host/db"
          className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1 flex-1 font-mono"
          onChange={(e) => onChange(e.target.value || null)}
        />
        <button
          type="button"
          className="px-3 py-1 border border-[var(--color-border)] text-xs"
          onClick={test}
          disabled={testResult.state === "testing"}
        >
          {testResult.state === "testing" ? "Testing…" : "Test connection"}
        </button>
        <button
          type="button"
          className="px-3 py-1 border border-[var(--color-border)] text-xs"
          onClick={useSqlite}
          title="Clears DATABASE_URL and saves settings. Restart the app to switch backends."
        >
          Use SQLite
        </button>
      </div>
      {cleared && !saveError && (
        <span className="text-xs text-[var(--color-good)]">
          ✓ Switched to SQLite — restart the app to take effect
        </span>
      )}
      {saveError && (
        <span className="text-xs text-[var(--color-bad)]">
          ✗ Save failed: {saveError}
        </span>
      )}
      {testResult.state === "ok" && (
        <span className="text-xs text-[var(--color-good)]">
          ✓ {testResult.msg}
        </span>
      )}
      {testResult.state === "fail" && (
        <span className="text-xs text-[var(--color-bad)]">
          ✗ {testResult.msg}
        </span>
      )}
    </>
  );
}

function SpanshMarketsControl() {
  const progress = useStore((s) => s.spanshProgress);
  const busy = useStore((s) => s.spanshBusy);

  async function start() {
    useStore.setState({ spanshBusy: true });
    try {
      await api.importSpanshMarkets();
    } catch (e) {
      console.error("markets import failed", e);
    } finally {
      useStore.setState({ spanshBusy: false });
    }
  }

  const active =
    progress &&
    (progress.phase === "downloading" || progress.phase === "importing");

  if (active && progress) {
    // UI is rendered by the shared progress control above (same events bus) —
    // so while galaxy OR markets are running, that component shows the bar.
    // Here we just stay disabled so the user can't start a second operation.
    return (
      <button
        type="button"
        className="px-3 py-1 border border-[var(--color-border)] text-[var(--color-text-dim)] text-xs"
        disabled
      >
        {progress.message ?? "Working…"}
      </button>
    );
  }

  return (
    <button
      type="button"
      className="px-3 py-1 border border-[var(--color-accent)] text-[var(--color-accent)] text-xs"
      onClick={start}
      disabled={busy}
    >
      {busy ? "Starting…" : "Import latest markets (~2 GB download)"}
    </button>
  );
}

function SpanshControl({ already }: { already: boolean }) {
  const progress = useStore((s) => s.spanshProgress);
  const busy = useStore((s) => s.spanshBusy);

  const active =
    progress &&
    (progress.phase === "downloading" || progress.phase === "importing");

  async function start() {
    useStore.setState({ spanshBusy: true });
    try {
      await api.downloadSpanshGalaxy();
      await useStore.getState().refreshSettings();
    } catch (e) {
      console.error("spansh download failed", e);
    } finally {
      useStore.setState({ spanshBusy: false });
    }
  }

  if (active && progress) {
    const done = Number(progress.bytes_done);
    const total = progress.bytes_total ? Number(progress.bytes_total) : null;
    const pct = total ? Math.min(100, (done / total) * 100) : null;
    const isImporting = progress.phase === "importing";
    const imported = Number(progress.systems_imported);
    const label = progress.message ?? (isImporting ? "Importing…" : "Downloading…");

    return (
      <div className="flex flex-col gap-1 min-w-[320px]">
        <div className="flex justify-between text-xs">
          <span className="text-[var(--color-text-dim)]">{label}</span>
          <span className="font-mono tabular-nums">
            {isImporting
              ? `${imported.toLocaleString()}`
              : pct !== null
                ? `${pct.toFixed(1)}%`
                : formatBytes(done)}
          </span>
        </div>
        <div className="h-1.5 bg-[var(--color-panel-hi)] rounded overflow-hidden">
          <div
            className="h-full bg-[var(--color-accent)] transition-[width] duration-150"
            style={{
              width: isImporting ? "100%" : pct !== null ? `${pct}%` : "30%",
              opacity: isImporting ? 0.5 : 1,
            }}
          />
        </div>
        {total && !isImporting && (
          <div className="text-xs text-[var(--color-text-dim)] font-mono">
            {formatBytes(done)} / {formatBytes(total)}
          </div>
        )}
      </div>
    );
  }

  if (already && !busy) {
    return <span className="text-[var(--color-good)]">downloaded</span>;
  }

  return (
    <button
      type="button"
      className="px-3 py-1 border border-[var(--color-accent)] text-[var(--color-accent)] text-xs"
      onClick={start}
      disabled={busy}
    >
      {busy ? "Starting…" : "Download Spansh galaxy (populated systems)"}
    </button>
  );
}

const WEIGHT_KEYS = [
  ["freshness", "Freshness"],
  ["niche", "Niche bonus"],
  ["fleet_carrier", "Fleet carrier"],
  ["reachability", "Reachability"],
] as const;

export default function SettingsView() {
  const settings = useStore((s) => s.settings);
  const setSettings = useStore((s) => s.setSettings);
  const diagnostics = useStore((s) => s.diagnostics);
  const refreshDiagnostics = useStore((s) => s.refreshDiagnostics);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved" | "discarded">("idle");

  useEffect(() => {
    setDraft(settings);
  }, [settings]);
  useEffect(() => {
    refreshDiagnostics();
    const id = setInterval(refreshDiagnostics, 3000);
    return () => clearInterval(id);
  }, [refreshDiagnostics]);

  if (!draft)
    return <div className="p-6 text-[var(--color-text-dim)]">loading…</div>;

  return (
    <div className="p-6 grid gap-8 max-w-2xl">
      <section>
        <h2 className="text-lg mb-3">
          Scoring weights (0 disables, 2 doubles influence)
        </h2>
        <div className="grid grid-cols-[140px_1fr_60px] gap-3 items-center text-sm">
          {WEIGHT_KEYS.map(([k, label]) => (
            <div key={k} className="contents">
              <label className="text-[var(--color-text-dim)]">{label}</label>
              <input
                type="range"
                min="0"
                max="2"
                step="0.05"
                value={(draft.score_weights as any)[k]}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    score_weights: {
                      ...draft.score_weights,
                      [k]: Number(e.target.value),
                    },
                  })
                }
              />
              <span className="tabular-nums font-mono text-xs">
                {((draft.score_weights as any)[k]).toFixed(2)}
              </span>
            </div>
          ))}
        </div>
        <button
          className="mt-3 text-xs text-[var(--color-text-dim)] underline"
          onClick={() =>
            setDraft({
              ...draft,
              score_weights: {
                freshness: 1,
                niche: 1,
                fleet_carrier: 1,
                reachability: 1,
              },
            })
          }
        >
          reset defaults
        </button>
      </section>

      <section>
        <h2 className="text-lg mb-3">Alerts</h2>
        <div className="grid grid-cols-[200px_1fr] gap-3 text-sm items-center">
          <label className="text-[var(--color-text-dim)]">Desktop toasts</label>
          <input
            type="checkbox"
            checked={draft.alerts.desktop_enabled}
            onChange={(e) =>
              setDraft({
                ...draft,
                alerts: { ...draft.alerts, desktop_enabled: e.target.checked },
              })
            }
          />

          <label className="text-[var(--color-text-dim)]">Min profit/ton</label>
          <input
            type="number"
            value={draft.alerts.min_profit_per_ton}
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1 w-40"
            onChange={(e) =>
              setDraft({
                ...draft,
                alerts: {
                  ...draft.alerts,
                  min_profit_per_ton: Number(e.target.value),
                },
              })
            }
          />

          <label className="text-[var(--color-text-dim)]">Min cr/hr</label>
          <input
            type="number"
            value={String(draft.alerts.min_cr_per_hour)}
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1 w-40"
            onChange={(e) =>
              setDraft({
                ...draft,
                alerts: {
                  ...draft.alerts,
                  min_cr_per_hour: BigInt(e.target.value || "0"),
                },
              })
            }
          />

          <label className="text-[var(--color-text-dim)]">Max distance (ly)</label>
          <input
            type="number"
            value={draft.alerts.max_distance_ly}
            step="0.5"
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1 w-40"
            onChange={(e) =>
              setDraft({
                ...draft,
                alerts: {
                  ...draft.alerts,
                  max_distance_ly: Number(e.target.value),
                },
              })
            }
          />

          <label className="text-[var(--color-text-dim)]">
            Cooldown (minutes)
          </label>
          <input
            type="number"
            value={draft.alerts.cooldown_minutes}
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1 w-40"
            onChange={(e) =>
              setDraft({
                ...draft,
                alerts: {
                  ...draft.alerts,
                  cooldown_minutes: Number(e.target.value),
                },
              })
            }
          />

          <label className="text-[var(--color-text-dim)]">Webhook URL</label>
          <input
            type="text"
            value={draft.alerts.webhook_url ?? ""}
            placeholder="https://…"
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1"
            onChange={(e) =>
              setDraft({
                ...draft,
                alerts: {
                  ...draft.alerts,
                  webhook_url: e.target.value || null,
                },
              })
            }
          />
        </div>
      </section>

      <section>
        <h2 className="text-lg mb-3">Data sources</h2>
        <div className="grid grid-cols-[200px_1fr] gap-3 text-sm items-center">
          <label className="text-[var(--color-text-dim)]">EDDN relay</label>
          <input
            type="text"
            value={draft.data_sources.eddn_relay_url}
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1"
            onChange={(e) =>
              setDraft({
                ...draft,
                data_sources: {
                  ...draft.data_sources,
                  eddn_relay_url: e.target.value,
                },
              })
            }
          />
          <label className="text-[var(--color-text-dim)]">Journal dir</label>
          <input
            type="text"
            value={draft.data_sources.journal_dir ?? ""}
            placeholder="auto-detected"
            className="bg-[var(--color-panel)] border border-[var(--color-border)] px-2 py-1"
            onChange={(e) =>
              setDraft({
                ...draft,
                data_sources: {
                  ...draft.data_sources,
                  journal_dir: e.target.value || null,
                },
              })
            }
          />
          <label className="text-[var(--color-text-dim)]">Spansh galaxy</label>
          <div>
            <SpanshControl
              already={draft.data_sources.spansh_galaxy_downloaded}
            />
          </div>
          <label className="text-[var(--color-text-dim)]">Import markets</label>
          <div className="flex flex-col gap-1">
            <SpanshMarketsControl />
            <span className="text-xs text-[var(--color-text-dim)]">
              Downloads <span className="font-mono">galaxy_stations.json.gz</span> (~2 GB) and imports the latest market snapshot for every station. Useful as a one-time bootstrap so you don't have to wait for EDDN to accumulate data. Safe to re-run — duplicate snapshots are skipped.
            </span>
          </div>
          <label className="text-[var(--color-text-dim)]">DATABASE_URL</label>
          <div className="flex flex-col gap-1">
            <DatabaseUrlControl
              value={draft.data_sources.database_url}
              onChange={(v) =>
                setDraft({
                  ...draft,
                  data_sources: { ...draft.data_sources, database_url: v },
                })
              }
            />
            <span className="text-xs text-[var(--color-text-dim)]">
              Leave blank for SQLite in app data dir. Postgres example:
              <span className="font-mono ml-1">
                postgres://user:pass@localhost/elite_trade
              </span>
              . Save + restart the app to switch backends.
            </span>
          </div>
        </div>
      </section>

      <section>
        <h2 className="text-lg mb-3">Storage</h2>
        <div className="grid grid-cols-[200px_1fr] gap-3 text-sm">
          <span className="text-[var(--color-text-dim)]">Backend</span>
          <span className="font-mono">{diagnostics?.db_dialect ?? "…"}</span>
          <span className="text-[var(--color-text-dim)]">Snapshot count</span>
          <span className="font-mono tabular-nums">
            {diagnostics?.snapshot_count?.toLocaleString() ?? "…"}
          </span>
          <span className="text-[var(--color-text-dim)]">Oldest / Newest</span>
          <span className="font-mono text-xs">
            {diagnostics?.oldest_snapshot
              ? new Date(diagnostics.oldest_snapshot).toISOString()
              : "—"}{" "}
            →{" "}
            {diagnostics?.newest_snapshot
              ? new Date(diagnostics.newest_snapshot).toISOString()
              : "—"}
          </span>
          <span className="text-[var(--color-text-dim)]">EDDN status</span>
          <span
            className={
              diagnostics?.eddn_connected
                ? "text-[var(--color-good)]"
                : "text-[var(--color-bad)]"
            }
          >
            {diagnostics?.eddn_connected ? "connected" : "disconnected"} ·{" "}
            {diagnostics?.eddn_msgs_per_sec?.toFixed(1) ?? "0.0"} msg/s
          </span>
          <span className="text-[var(--color-text-dim)]">Journal status</span>
          <span className="font-mono">{diagnostics?.journal_status}</span>
        </div>
        <div className="mt-3 flex gap-2 items-start flex-wrap">
          <button
            type="button"
            className="px-3 py-1 border border-[var(--color-border)] text-xs"
            onClick={async () => {
              const [s, a] = await api.forcePrune();
              alert(`Pruned ${s} snapshots, ${a} alerts`);
            }}
          >
            Run retention prune now
          </button>
          <button
            type="button"
            className="px-3 py-1 border border-[var(--color-border)] text-xs"
            onClick={async () => {
              try {
                const n = await api.rebuildLatestMarket();
                alert(
                  `latest_market rebuilt: ${n.toLocaleString()} rows`,
                );
              } catch (e) {
                alert(`Rebuild failed: ${e}`);
              }
            }}
          >
            Rebuild latest-market cache
          </button>
          <DiagnoseRoutes />
        </div>
      </section>

      <div className="sticky bottom-0 bg-[var(--color-bg)] border-t border-[var(--color-border)] py-3 flex gap-3 items-center">
        <button
          type="button"
          className="px-4 py-2 bg-[var(--color-accent)] text-black text-sm font-medium disabled:opacity-50"
          disabled={saveStatus === "saving"}
          onClick={async () => {
            setSaveStatus("saving");
            try {
              await setSettings(draft);
              setSaveStatus("saved");
              setTimeout(() => setSaveStatus("idle"), 2000);
            } catch (e) {
              console.error("save failed", e);
              setSaveStatus("idle");
            }
          }}
        >
          {saveStatus === "saving" ? "Saving…" : "Save settings"}
        </button>
        <button
          type="button"
          className="px-4 py-2 border border-[var(--color-border)] text-sm"
          onClick={() => {
            setDraft(settings);
            setSaveStatus("discarded");
            setTimeout(() => setSaveStatus("idle"), 2000);
          }}
        >
          Discard changes
        </button>
        {saveStatus === "saved" && (
          <span className="text-xs text-[var(--color-good)]">✓ Saved</span>
        )}
        {saveStatus === "discarded" && (
          <span className="text-xs text-[var(--color-text-dim)]">
            Reverted to last saved
          </span>
        )}
      </div>
    </div>
  );
}
