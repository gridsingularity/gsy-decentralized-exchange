// Grid topology view (Milestone 3).
//
// Flow: communities list -> pick a community -> its topology tree + the list of
// its markets (one per time_slot) -> pick a market -> Market view placeholder.
//
// Communities come from GET /communities when available. Against the current
// (older) backend that endpoint 404s, so we FALL BACK to deriving the list
// client-side from GET /markets in the selected window, grouped by the stable
// `community_name`.

import { useEffect, useState } from 'react';
import {
  getCommunities,
  getCommunityMarket,
  getMarketsInWindow,
} from '../api/client';
import type { CommunitySummary, MarketTopologySchema } from '../api/schema';
import { formatSlot, type Window } from '../lib/time';
import { truncate } from '../lib/format';
import TopologyTree from '../components/TopologyTree';
import { Empty, ErrorNote, Loading } from '../components/AsyncState';
import MarketView from './MarketView';
import AssetView from './AssetView';

interface Props {
  window: Window;
  community: string | undefined;
  market: string | undefined;
  asset: string | undefined;
  onSelectCommunity: (name: string | undefined) => void;
  onSelectMarket: (marketId: string | undefined) => void;
  onSelectAsset: (name: string) => void;
}

type CommunitiesSource = 'communities' | 'markets-fallback';

interface CommunitiesState {
  loading: boolean;
  error: string | null;
  source: CommunitiesSource;
  items: CommunitySummary[];
}

/** Reduce a flat list of markets into per-community summaries (fallback path). */
function deriveCommunities(
  markets: MarketTopologySchema[],
): CommunitySummary[] {
  const byName = new Map<string, CommunitySummary>();
  for (const m of markets) {
    const existing = byName.get(m.community_name);
    if (!existing) {
      byName.set(m.community_name, {
        community_name: m.community_name,
        community_uuid: m.community_uuid,
        market_count: 1,
        earliest_slot: m.time_slot,
        latest_slot: m.time_slot,
      });
    } else {
      existing.market_count += 1;
      existing.earliest_slot = Math.min(existing.earliest_slot, m.time_slot);
      existing.latest_slot = Math.max(existing.latest_slot, m.time_slot);
    }
  }
  return [...byName.values()].sort((a, b) =>
    a.community_name.localeCompare(b.community_name),
  );
}

export default function GridView({
  window,
  community,
  market,
  asset,
  onSelectCommunity,
  onSelectMarket,
  onSelectAsset,
}: Props) {
  const [communities, setCommunities] = useState<CommunitiesState>({
    loading: true,
    error: null,
    source: 'communities',
    items: [],
  });

  // Load communities (primary endpoint, then markets-derived fallback).
  useEffect(() => {
    let cancelled = false;
    setCommunities((s) => ({ ...s, loading: true, error: null }));
    (async () => {
      try {
        const items = await getCommunities();
        if (cancelled) return;
        setCommunities({
          loading: false,
          error: null,
          source: 'communities',
          items,
        });
      } catch {
        // /communities unavailable (e.g. older backend) — derive from /markets.
        try {
          const markets = await getMarketsInWindow(window.start, window.end);
          if (cancelled) return;
          setCommunities({
            loading: false,
            error: null,
            source: 'markets-fallback',
            items: deriveCommunities(markets),
          });
        } catch (err2) {
          if (cancelled) return;
          setCommunities({
            loading: false,
            error: err2 instanceof Error ? err2.message : String(err2),
            source: 'markets-fallback',
            items: [],
          });
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [window.start, window.end]);

  return (
    <div className="grid-view">
      <aside className="pane pane-communities">
        <h2>Communities</h2>
        {communities.source === 'markets-fallback' &&
          communities.error === null && (
            <p className="banner">
              /communities unavailable — showing communities derived from
              /markets in the selected window
            </p>
          )}
        {communities.loading && <Loading label="Loading communities…" />}
        {communities.error && <ErrorNote error={communities.error} />}
        {!communities.loading &&
          !communities.error &&
          communities.items.length === 0 && (
            <Empty>No communities in this window.</Empty>
          )}
        <ul className="community-list">
          {communities.items.map((c) => (
            <li key={c.community_name}>
              <button
                type="button"
                className={
                  c.community_name === community ? 'row-btn selected' : 'row-btn'
                }
                onClick={() => {
                  onSelectCommunity(c.community_name);
                  onSelectMarket(undefined);
                }}
              >
                <span className="community-name">{c.community_name}</span>
                <span className="muted">
                  {c.market_count} market(s) · {formatSlot(c.earliest_slot)} →{' '}
                  {formatSlot(c.latest_slot)}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </aside>

      <section className="pane pane-detail">
        {community ? (
          <CommunityDetail
            window={window}
            community={community}
            market={market}
            asset={asset}
            onSelectMarket={onSelectMarket}
            onSelectAsset={onSelectAsset}
          />
        ) : (
          <Empty>Select a community to view its topology.</Empty>
        )}
      </section>
    </div>
  );
}

interface DetailProps {
  window: Window;
  community: string;
  market: string | undefined;
  asset: string | undefined;
  onSelectMarket: (marketId: string | undefined) => void;
  onSelectAsset: (name: string) => void;
}

interface MarketsState {
  loading: boolean;
  error: string | null;
  items: MarketTopologySchema[];
}

function CommunityDetail({
  window,
  community,
  market,
  asset,
  onSelectMarket,
  onSelectAsset,
}: DetailProps) {
  const [state, setState] = useState<MarketsState>({
    loading: true,
    error: null,
    items: [],
  });

  useEffect(() => {
    let cancelled = false;
    setState({ loading: true, error: null, items: [] });
    (async () => {
      try {
        const items = await getCommunityMarket(community, {
          start_time: window.start,
          end_time: window.end,
        });
        if (cancelled) return;
        setState({ loading: false, error: null, items });
      } catch (err) {
        if (cancelled) return;
        setState({
          loading: false,
          error: err instanceof Error ? err.message : String(err),
          items: [],
        });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [community, window.start, window.end]);

  if (state.loading) return <Loading label="Loading markets…" />;
  if (state.error) return <ErrorNote error={state.error} />;
  if (state.items.length === 0) {
    return (
      <Empty>No markets for “{community}” in the selected window.</Empty>
    );
  }

  // Canonical topology = the most-recent market's areas (max time_slot).
  const sorted = [...state.items].sort((a, b) => b.time_slot - a.time_slot);
  const canonical = sorted[0];

  return (
    <div className="detail-grid">
      <div className="detail-col">
        <h2>{community}</h2>
        <h3>Topology (most recent slot)</h3>
        <TopologyTree
          areas={canonical.community_areas}
          onSelectAsset={onSelectAsset}
          selectedAsset={asset}
        />

        <h3>Markets ({sorted.length})</h3>
        <ul className="slot-list">
          {sorted.map((m) => (
            <li key={m.market_id}>
              <button
                type="button"
                className={
                  m.market_id === market ? 'row-btn selected' : 'row-btn'
                }
                onClick={() => onSelectMarket(m.market_id)}
              >
                <span>{formatSlot(m.time_slot)}</span>
                <span className="mono muted">{truncate(m.market_id)}</span>
                <span className="muted">
                  {m.community_areas.length} asset(s)
                </span>
              </button>
            </li>
          ))}
        </ul>
      </div>

      <div className="detail-col">
        {asset ? (
          <AssetView markets={sorted} assetName={asset} window={window} />
        ) : market ? (
          <MarketView
            markets={sorted}
            selectedMarketId={market}
            window={window}
          />
        ) : (
          <Empty>
            Select a market slot to open its (M4) view, or an asset in the
            topology to open its (M5) view.
          </Empty>
        )}
      </div>
    </div>
  );
}
