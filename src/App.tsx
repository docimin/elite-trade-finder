import { useEffect, useState } from "react";
import Sidebar, { type View } from "./components/Sidebar";
import LiveRoutes from "./views/LiveRoutes";
import Firehose from "./views/Firehose";
import Commander from "./views/Commander";
import SettingsView from "./views/Settings";
import { useStore } from "./store";

export default function App() {
  const [view, setView] = useState<View>("routes");
  const initEventListeners = useStore((s) => s.initEventListeners);
  const refreshSettings = useStore((s) => s.refreshSettings);
  const refreshUserState = useStore((s) => s.refreshUserState);

  useEffect(() => {
    let unsub: (() => void) | null = null;
    let cancelled = false;

    async function boot() {
      // Rust bootstrap() runs async in parallel with this mount. Retry until
      // app.manage(state) has been called so the Tauri commands stop throwing.
      for (let attempt = 0; attempt < 60 && !cancelled; attempt++) {
        try {
          await refreshSettings();
          await refreshUserState();
          unsub = await initEventListeners();
          return;
        } catch {
          await new Promise((r) => setTimeout(r, 500));
        }
      }
      console.warn("UI bootstrap timed out waiting for backend");
    }

    boot();
    return () => {
      cancelled = true;
      if (unsub) unsub();
    };
  }, [initEventListeners, refreshSettings, refreshUserState]);

  return (
    <div className="h-full flex">
      <Sidebar active={view} onSelect={setView} />
      <main className="flex-1 overflow-auto">
        {view === "routes" && <LiveRoutes />}
        {view === "firehose" && <Firehose />}
        {view === "commander" && <Commander />}
        {view === "settings" && <SettingsView />}
      </main>
    </div>
  );
}
