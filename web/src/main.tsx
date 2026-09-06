import { render } from 'preact';
import { useEffect, useState } from 'preact/hooks';
import type { Overview } from './bindings/Overview';
import './style.css';

function OverviewDetails({ overview }: { overview: Overview }) {
  return (
    <dl class="panel">
      <dt>Project path</dt>
      <dd>{overview.project_path}</dd>
      <dt>Memory size</dt>
      <dd>{overview.mem_size.toLocaleString()} bytes</dd>
      <dt>Executable</dt>
      <dd>{overview.exe.filename}</dd>
      <dt>Entry point</dt>
      <dd><code>{overview.exe.entry_point}</code></dd>
    </dl>
  );
}

function FunctionList({ functions }: { functions: Overview['functions'] }) {
  return (
    <section class="panel" aria-labelledby="functions-heading">
      <h2 id="functions-heading">Functions ({functions.length})</h2>
      {functions.length === 0 ? (
        <p>No functions found.</p>
      ) : (
        <table>
          <thead>
            <tr><th scope="col">IP</th><th scope="col">Name</th></tr>
          </thead>
          <tbody>
            {functions.map(func => (
              <tr key={func.ip}>
                <td><code>{func.ip}</code></td>
                <td>{func.name ?? 'Unnamed'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

function App() {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();

    async function loadOverview() {
      try {
        const response = await fetch('/api/overview', {
          signal: controller.signal,
        });
        if (!response.ok) {
          throw new Error(`Request failed (${response.status})`);
        }
        const data: Overview = await response.json();
        if (!controller.signal.aborted) setOverview(data);
      } catch (error) {
        if (!controller.signal.aborted) {
          setError(error instanceof Error ? error.message : String(error));
        }
      }
    }

    void loadOverview();
    return () => controller.abort();
  }, []);

  return (
    <main>
      <h1>Project overview</h1>
      {error ? (
        <p class="panel" role="alert">Could not load overview: {error}</p>
      ) : overview === null ? (
        <p class="panel" role="status">Loading overview…</p>
      ) : (
        <>
          <OverviewDetails overview={overview} />
          <FunctionList functions={overview.functions} />
        </>
      )}
    </main>
  );
}

render(<App />, document.getElementById('app')!);
