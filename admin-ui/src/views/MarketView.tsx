// Aggregated Market view (Milestone 4).
//
// SCOPE: a `market_id` in this data model is a single (community, time_slot)
// pair — each community has ~one market per slot. "Per time slot" trends are
// therefore only meaningful ACROSS a community's slots, so this view is scoped
// to the whole community across the window's slots, with the grid-selected slot
// highlighted (NOT a single market_id).
//
// Data flow: fetch trades once for the window, then filter client-side to the
// community's set of market_ids (trade volume is tiny), group by time_slot.

import { useEffect, useMemo, useState } from 'react';
import { getTradesResolved } from '../api/client';
import type {
  MarketTopologySchema,
  TradeCanonicalSchema,
} from '../api/schema';
import { perSlotAggregates } from '../lib/aggregate';
import { formatSlot, type Window } from '../lib/time';
import SlotAggregatesTable from '../components/SlotAggregatesTable';
import SlotBarChart, { type BarPoint } from '../components/SlotBarChart';
import RawTradesTable from '../components/RawTradesTable';
import { Empty, ErrorNote, Loading } from '../components/AsyncState';

interface Props {
  markets: MarketTopologySchema[]; // one per slot, sorted by time_slot desc
  selectedMarketId: string | undefined;
  window: Window;
}

interface TradesState {
  loading: boolean;
  error: string | null;
  trades: TradeCanonicalSchema[];
}

/** Short HH:mm label for a slot, used on chart x-axes. */
function slotLabel(sec: number): string {
  const d = new Date(sec * 1000);
  return `${String(d.getHours()).padStart(2, '0')}:${String(
    d.getMinutes(),
  ).padStart(2, '0')}`;
}

export default function MarketView({
  markets,
  selectedMarketId,
  window,
}: Props) {
  const [state, setState] = useState<TradesState>({
    loading: true,
    error: null,
    trades: [],
  });

  const marketIds = useMemo(
    () => new Set(markets.map((m) => m.market_id)),
    [markets],
  );
  const selectedSlot = useMemo(
    () => markets.find((m) => m.market_id === selectedMarketId)?.time_slot,
    [markets, selectedMarketId],
  );
  const communityName = markets[0]?.community_name ?? '';

  // Fetch trades once for the window, filter to this community's markets.
  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, trades: [] });
    (async () => {
      try {
        const all = await getTradesResolved({
          start_time: window.start,
          end_time: window.end,
        });
        if (cancelled) return;
        const trades = all.filter((t) => marketIds.has(t.market_id));
        setState({ loading: false, error: null, trades });
      } catch (err) {
        if (cancelled) return;
        setState({
          loading: false,
          error: err instanceof Error ? err.message : String(err),
          trades: [],
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [marketIds, window.start, window.end]);

  const aggregates = useMemo(
    () => perSlotAggregates(state.trades),
    [state.trades],
  );

  const totalVolume = useMemo(
    () => aggregates.reduce((s, a) => s + a.volume, 0),
    [aggregates],
  );

  const volumePoints: BarPoint[] = aggregates.map((a) => ({
    label: slotLabel(a.time_slot),
    value: a.volume,
    highlight: a.time_slot === selectedSlot,
  }));

  const ratePoints: BarPoint[] = aggregates.map((a) => ({
    label: slotLabel(a.time_slot),
    value: a.avgRatePerKwh ?? 0,
    highlight: a.time_slot === selectedSlot,
  }));

  return (
    <section className="market-view">
      <header className="market-header">
        <h2>{communityName || 'Market'}</h2>
        <dl className="market-stats">
          <div>
            <dt>Slots</dt>
            <dd>{markets.length}</dd>
          </div>
          <div>
            <dt>Selected slot</dt>
            <dd>
              {selectedSlot !== undefined ? (
                <span className="pill selected">{formatSlot(selectedSlot)}</span>
              ) : (
                <span className="muted">none</span>
              )}
            </dd>
          </div>
          <div>
            <dt>Trades (window)</dt>
            <dd>{state.trades.length}</dd>
          </div>
          <div>
            <dt>Volume (window)</dt>
            <dd>{totalVolume.toFixed(3)} kWh</dd>
          </div>
        </dl>
      </header>

      {state.loading && <Loading label="Loading trades…" />}
      {state.error && <ErrorNote error={state.error} />}

      {!state.loading && !state.error && (
        <>
          {state.trades.length === 0 ? (
            <Empty>
              No trades for “{communityName}” in the selected window.
            </Empty>
          ) : (
            <>
              <h3>Per-slot aggregates</h3>
              <SlotAggregatesTable
                aggregates={aggregates}
                selectedSlot={selectedSlot}
              />

              <div className="chart-row">
                <SlotBarChart
                  points={volumePoints}
                  title="Traded volume per slot"
                  unit="kWh"
                />
                <SlotBarChart
                  points={ratePoints}
                  title="Avg rate per slot"
                  unit="price/kWh"
                />
              </div>

              <h3>Raw trades ({state.trades.length})</h3>
              <RawTradesTable trades={state.trades} />
            </>
          )}
        </>
      )}
    </section>
  );
}
