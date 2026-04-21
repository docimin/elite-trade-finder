import { useStore } from "../store";

export default function Firehose() {
  const firehose = useStore((s) => s.firehose);
  const alerts = useStore((s) => s.recentAlerts);
  const paused = useStore((s) => s.firehosePaused);
  const togglePause = useStore((s) => s.togglePauseFirehose);

  return (
    <div className="p-4 h-full flex flex-col gap-4">
      <div className="flex items-center">
        <h2 className="text-lg">Firehose</h2>
        <button
          onClick={togglePause}
          className="ml-auto px-3 py-1 border border-[var(--color-border)] text-xs"
        >
          {paused ? "Resume" : "Pause"}
        </button>
      </div>

      <div className="grid grid-cols-2 gap-4 flex-1 min-h-0">
        <div className="flex flex-col min-h-0">
          <div className="text-xs text-[var(--color-text-dim)] uppercase mb-2">
            Stream
          </div>
          <div className="flex-1 overflow-auto border border-[var(--color-border)] font-mono text-xs">
            {firehose.map((t, i) => (
              <div
                key={i}
                className="px-2 py-1 border-b border-[var(--color-border)] flex gap-3"
              >
                <span className="text-[var(--color-text-dim)]">
                  {new Date(t.at).toLocaleTimeString()}
                </span>
                <span>{t.system}</span>
                <span className="text-[var(--color-text-dim)]">
                  / {t.station}
                </span>
                <span className="ml-auto text-[var(--color-good)]">
                  {t.commodities_updated} updated
                </span>
              </div>
            ))}
            {firehose.length === 0 && (
              <div className="p-4 text-center text-[var(--color-text-dim)]">
                waiting for messages…
              </div>
            )}
          </div>
        </div>

        <div className="flex flex-col min-h-0">
          <div className="text-xs text-[var(--color-text-dim)] uppercase mb-2">
            Recent alerts
          </div>
          <div className="flex-1 overflow-auto border border-[var(--color-border)] font-mono text-xs">
            {alerts.map((r, i) => (
              <div
                key={i}
                className="px-2 py-1 border-b border-[var(--color-border)]"
              >
                <div className="text-[var(--color-accent)]">
                  {(Number(r.cr_per_hour) / 1_000_000).toFixed(1)}M cr/hr · {r.mode}
                </div>
                <div className="text-[var(--color-text-dim)]">
                  {r.legs[0]?.from_station} →{" "}
                  {r.legs[r.legs.length - 1]?.to_station}
                </div>
              </div>
            ))}
            {alerts.length === 0 && (
              <div className="p-4 text-center text-[var(--color-text-dim)]">
                no alerts yet
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
