import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  RankedRoute,
  RouteFilter,
  UserState,
  Settings,
  ShipSpec,
  Diagnostics,
  FirehoseTick,
  SpanshProgress,
} from "../types";

export const api = {
  getTopRoutes: (filter: RouteFilter) =>
    invoke<RankedRoute[]>("get_top_routes", { filter }),
  getUserState: () => invoke<UserState>("get_user_state"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (newSettings: Settings) =>
    invoke<void>("set_settings", { newSettings }),
  manualOverrideShip: (ship: ShipSpec | null) =>
    invoke<void>("manual_override_ship", { ship }),
  forcePrune: () => invoke<[number, number]>("force_prune"),
  getDiagnostics: () => invoke<Diagnostics>("get_diagnostics"),
  downloadSpanshGalaxy: () => invoke<number>("download_spansh_galaxy"),
  testDatabaseUrl: (url: string) =>
    invoke<string>("test_database_url", { url }),
  debugRoutePipeline: () => invoke<string>("debug_route_pipeline"),
  importSpanshMarkets: () => invoke<number>("import_spansh_markets"),
  rebuildLatestMarket: () => invoke<number>("rebuild_latest_market"),
};

type EventMap = {
  routes_updated: RankedRoute[];
  route_alert: RankedRoute;
  firehose_tick: FirehoseTick;
  user_state_changed: UserState;
  spansh_progress: SpanshProgress;
};

export function on<K extends keyof EventMap>(
  event: K,
  handler: (payload: EventMap[K]) => void,
): Promise<UnlistenFn> {
  return listen<EventMap[K]>(event, (e) => handler(e.payload));
}
