// Shared presentational helpers for consistent loading / error / empty states
// across every data-fetching component (Milestone 6 hardening).
//
// Deliberately tiny and library-free: each just wraps the existing CSS classes
// (.muted, .error, .banner) so no fetch ever renders a blank region. Using
// these keeps the three async states visually and structurally consistent.

import type { ReactNode } from 'react';

/** Muted "Loading…" line shown while a fetch is in flight. */
export function Loading({ label = 'Loading…' }: { label?: string }) {
  return <p className="muted">{label}</p>;
}

/** Readable, selectable error block (never a raw throw / blank page). */
export function ErrorNote({ error }: { error: string }) {
  return <pre className="error">{error}</pre>;
}

/** Muted empty-state message shown when a fetch succeeds with no rows. */
export function Empty({ children }: { children: ReactNode }) {
  return <p className="muted">{children}</p>;
}
