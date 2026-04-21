import { create } from "zustand";
import type {
  RankedRoute,
  RouteMode,
  UserState,
  Settings,
  Diagnostics,
  FirehoseTick,
  SpanshProgress,
} from "../types";

export type FcPolicy = "any" | "exclude" | "require";

export type RouteFilterState = {
  maxJumps: number;
  minCrHr: number;
  maxPpt: number;
  modes: RouteMode[];
  fcPolicy: FcPolicy;
};

const ALL_MODES: RouteMode[] = ["single", "loop2", "loop3", "loop4", "rare_chain"];

const DEFAULT_ROUTE_FILTER: RouteFilterState = {
  maxJumps: 20,
  minCrHr: 0,
  maxPpt: 300_000,
  modes: ALL_MODES,
  fcPolicy: "any",
};
import { api, on } from "../api/tauri";

type AppStore = {
  routes: RankedRoute[];
  userState: UserState | null;
  settings: Settings | null;
  diagnostics: Diagnostics | null;
  firehose: FirehoseTick[];
  recentAlerts: RankedRoute[];
  firehosePaused: boolean;
  spanshProgress: SpanshProgress | null;
  spanshBusy: boolean;
  routeFilter: RouteFilterState;

  refreshSettings: () => Promise<void>;
  refreshUserState: () => Promise<void>;
  refreshDiagnostics: () => Promise<void>;
  setSettings: (s: Settings) => Promise<void>;
  togglePauseFirehose: () => void;
  initEventListeners: () => Promise<() => void>;
};

export const useStore = create<AppStore>((set, get) => ({
  routes: [],
  userState: null,
  settings: null,
  diagnostics: null,
  firehose: [],
  recentAlerts: [],
  firehosePaused: false,
  spanshProgress: null,
  spanshBusy: false,
  routeFilter: { ...DEFAULT_ROUTE_FILTER },

  refreshSettings: async () => set({ settings: await api.getSettings() }),
  refreshUserState: async () => set({ userState: await api.getUserState() }),
  refreshDiagnostics: async () =>
    set({ diagnostics: await api.getDiagnostics() }),
  setSettings: async (s) => {
    await api.setSettings(s);
    set({ settings: s });
  },
  togglePauseFirehose: () => set({ firehosePaused: !get().firehosePaused }),

  initEventListeners: async () => {
    const offs = await Promise.all([
      on("routes_updated", (r) => set({ routes: r })),
      on("user_state_changed", (u) => set({ userState: u })),
      on("firehose_tick", (t) => {
        if (get().firehosePaused) return;
        set((st) => ({ firehose: [t, ...st.firehose].slice(0, 200) }));
      }),
      on("route_alert", (r) => {
        set((st) => ({ recentAlerts: [r, ...st.recentAlerts].slice(0, 50) }));
      }),
      on("spansh_progress", (p) => set({ spanshProgress: p })),
    ]);
    return () => offs.forEach((f) => f());
  },
}));
