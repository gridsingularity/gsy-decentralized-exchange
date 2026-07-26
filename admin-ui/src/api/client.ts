// Tiny typed fetch wrapper around the gsy-offchain-storage REST API.
//
// Base URL: configurable via the `VITE_API_BASE_URL` build-time env var.
//   - Unset (dev default) -> `/api`, which the Vite dev proxy rewrites to the
//     backend on http://localhost:8080 (same-origin in dev, no CORS needed).
//   - Set to a full origin (e.g. `http://some-host:8080`) -> calls go directly
//     there. Running the built app against a remote backend that way requires
//     that backend to allow this origin (CORS) — out of scope for this UI.
//
// Time bounds (start_time/end_time) are Unix seconds and must never exceed
// u32::MAX — the backend types them as u32 and would silently truncate.

import type {
  CommunitySummary,
  DbOrderSchema,
  ForecastSchema,
  MarketTopologySchema,
  MeasurementSchema,
  TradeCanonicalSchema,
  TradeSchema,
  TradedEnergyResponse,
} from './schema';

const API_BASE = import.meta.env.VITE_API_BASE_URL ?? '/api';

export type QueryParams = Record<
  string,
  string | number | boolean | undefined
>;

function buildQuery(params?: QueryParams): string {
  if (!params) return '';
  const usp = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) usp.append(key, String(value));
  }
  const qs = usp.toString();
  return qs ? `?${qs}` : '';
}

export class ApiError extends Error {
  readonly status: number;
  readonly path: string;

  constructor(status: number, path: string, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.path = path;
  }
}

export async function apiGet<T>(path: string, params?: QueryParams): Promise<T> {
  const url = `${API_BASE}${path}${buildQuery(params)}`;
  const res = await fetch(url);
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new ApiError(
      res.status,
      path,
      `GET ${url} failed: ${res.status} ${res.statusText} ${body}`.trim(),
    );
  }
  return (await res.json()) as T;
}

// --- Endpoints -----------------------------------------------------------

export interface TimeWindow {
  start_time?: number;
  end_time?: number;
}

/** GET /health_check — returns true if the backend answered 200. */
export async function getHealth(): Promise<boolean> {
  const res = await fetch(`${API_BASE}/health_check`);
  return res.ok;
}

/** GET /trades?market_id?&start_time?&end_time? */
export function getTrades(
  params?: { market_id?: string } & TimeWindow,
): Promise<TradeSchema[]> {
  return apiGet<TradeSchema[]>('/trades', { ...params });
}

/**
 * GET /trades-canonical?market_id?&start_time?&end_time?
 * Same params as getTrades, but each trade is enriched with resolved
 * seller_name/buyer_name asset names (null when unresolvable).
 */
export function getTradesCanonical(
  params?: { market_id?: string } & TimeWindow,
): Promise<TradeCanonicalSchema[]> {
  return apiGet<TradeCanonicalSchema[]>('/trades-canonical', { ...params });
}

/**
 * Resilient trades fetch used by the views. Prefers /trades-canonical, but
 * falls back to /trades (mapping names to null) when the backend does not yet
 * expose the canonical endpoint — e.g. an older Docker container that 404s.
 */
export async function getTradesResolved(
  params?: { market_id?: string } & TimeWindow,
): Promise<TradeCanonicalSchema[]> {
  try {
    return await getTradesCanonical(params);
  } catch {
    const trades = await getTrades(params);
    return trades.map((t) => ({ ...t, seller_name: null, buyer_name: null }));
  }
}

/** GET /communities — enumerate all communities (keyed on community_name). */
export function getCommunities(): Promise<CommunitySummary[]> {
  return apiGet<CommunitySummary[]>('/communities');
}

/** GET /markets?start_time=&end_time= — both params REQUIRED (u32 seconds). */
export function getMarketsInWindow(
  start_time: number,
  end_time: number,
): Promise<MarketTopologySchema[]> {
  return apiGet<MarketTopologySchema[]>('/markets', { start_time, end_time });
}

/** GET /market?market_id= — single market (404 if none, 500 if >1). */
export function getMarket(market_id: string): Promise<MarketTopologySchema> {
  return apiGet<MarketTopologySchema>('/market', { market_id });
}

/** GET /community-market?community_name=&start_time?&end_time? */
export function getCommunityMarket(
  community_name: string,
  window?: TimeWindow,
): Promise<MarketTopologySchema[]> {
  return apiGet<MarketTopologySchema[]>('/community-market', {
    community_name,
    ...window,
  });
}

/** GET /orders?market_id?&start_time?&end_time? */
export function getOrders(
  params?: { market_id?: string } & TimeWindow,
): Promise<DbOrderSchema[]> {
  return apiGet<DbOrderSchema[]>('/orders', { ...params });
}

/** GET /measurements?area_uuid?&start_time?&end_time? */
export function getMeasurements(
  params?: { area_uuid?: string } & TimeWindow,
): Promise<MeasurementSchema[]> {
  return apiGet<MeasurementSchema[]>('/measurements', { ...params });
}

/** GET /forecasts?area_uuid?&start_time?&end_time? */
export function getForecasts(
  params?: { area_uuid?: string } & TimeWindow,
): Promise<ForecastSchema[]> {
  return apiGet<ForecastSchema[]>('/forecasts', { ...params });
}

/** GET /traded-energy?id=<area_hash>&start_time?&end_time? */
export function getTradedEnergy(
  id: string,
  window?: TimeWindow,
): Promise<TradedEnergyResponse> {
  return apiGet<TradedEnergyResponse>('/traded-energy', { id, ...window });
}
