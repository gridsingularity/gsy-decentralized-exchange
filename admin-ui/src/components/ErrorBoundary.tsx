// Top-level React error boundary (Milestone 6 hardening).
//
// Class component because catching render errors requires the class-only
// lifecycle hooks (getDerivedStateFromError / componentDidCatch). Without this,
// an uncaught render error unmounts the whole tree and leaves a blank page —
// unhelpful for a debugging tool. Here we show the error message and a hint to
// reload instead.

import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // Surface the full component stack in the console for debugging.
    console.error('Unhandled render error:', error, info.componentStack);
  }

  render(): ReactNode {
    const { error } = this.state;
    if (!error) return this.props.children;

    return (
      <main className="app">
        <header className="app-header">
          <h1>GSY DEX Admin</h1>
          <span className="badge err">crashed</span>
        </header>
        <div className="error-boundary">
          <h2>Something went wrong.</h2>
          <p className="muted">
            The UI hit an unexpected error while rendering. Reload the page to
            recover; if it persists, check the browser console for the full
            component stack.
          </p>
          <pre className="error">{error.message || String(error)}</pre>
        </div>
      </main>
    );
  }
}
