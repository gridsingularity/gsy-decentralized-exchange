// App shell for the admin-ui. Single page, no router library: the whole
// navigable state (time window + selected community + selected market) lives in
// URL query params. Milestone 3 renders the grid topology view.

import { getHealth } from './api/client';
import { useQueryParams } from './lib/url';
import { parseWindow, type Window } from './lib/time';
import TimeWindowSelector from './components/TimeWindowSelector';
import GridView from './views/GridView';
import { useEffect, useState } from 'react';

export default function App() {
  const [params, setParams] = useQueryParams();
  const window: Window = parseWindow(params.start, params.end);

  const [healthy, setHealthy] = useState<boolean | null>(null);
  useEffect(() => {
    let cancelled = false;
    getHealth()
      .then((ok) => !cancelled && setHealthy(ok))
      .catch(() => !cancelled && setHealthy(false));
    return () => {
      cancelled = true;
    };
  }, []);

  const setWindow = (next: Window) => {
    setParams({ start: String(next.start), end: String(next.end) });
  };

  const badgeClass =
    healthy === null ? 'badge' : healthy ? 'badge ok' : 'badge err';
  const badgeLabel = healthy === null ? '…' : healthy ? 'OK' : 'ERROR';

  return (
    <main className="app">
      <header className="app-header">
        <h1>GSY DEX Admin</h1>
        <span className={badgeClass}>health_check: {badgeLabel}</span>
      </header>

      <TimeWindowSelector window={window} onChange={setWindow} />

      <GridView
        window={window}
        community={params.community}
        market={params.market}
        asset={params.asset}
        onSelectCommunity={(name) =>
          setParams({ community: name, market: undefined, asset: undefined })
        }
        onSelectMarket={(marketId) =>
          setParams({ market: marketId, asset: undefined })
        }
        onSelectAsset={(name) => setParams({ asset: name, market: undefined })}
      />
    </main>
  );
}
