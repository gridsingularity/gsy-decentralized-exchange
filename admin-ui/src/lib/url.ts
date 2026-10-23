// Minimal URL-query-param state, no router library. The admin-ui is a
// single page whose whole navigable state (time window, selected community,
// selected market) lives in `?key=value` params so views are bookmarkable and
// shareable. Reads on mount + `popstate`; writes via `history.pushState`.

import { useCallback, useEffect, useState } from 'react';

export type QueryState = Record<string, string | undefined>;

function readAll(): QueryState {
  const usp = new URLSearchParams(window.location.search);
  const out: QueryState = {};
  usp.forEach((value, key) => {
    out[key] = value;
  });
  return out;
}

export function useQueryParams(): [
  QueryState,
  (patch: QueryState) => void,
] {
  const [state, setState] = useState<QueryState>(() => readAll());

  useEffect(() => {
    const onPop = () => setState(readAll());
    window.addEventListener('popstate', onPop);
    return () => window.removeEventListener('popstate', onPop);
  }, []);

  const update = useCallback((patch: QueryState) => {
    const usp = new URLSearchParams(window.location.search);
    for (const [key, value] of Object.entries(patch)) {
      if (value === undefined || value === '') usp.delete(key);
      else usp.set(key, value);
    }
    const qs = usp.toString();
    const url = `${window.location.pathname}${qs ? `?${qs}` : ''}`;
    window.history.pushState(null, '', url);
    setState(readAll());
  }, []);

  return [state, update];
}
