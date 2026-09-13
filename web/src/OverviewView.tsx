import type { Overview } from './bindings/Overview';
import { useJson } from './useJson';

function OverviewDetails({ overview }: { overview: Overview }) {
  return (
    <dl class="panel">
      <dt>Executable</dt>
      <dd>{overview.exe.filename}</dd>
      <dt>Memory size</dt>
      <dd>{overview.mem_size.toLocaleString()} bytes</dd>
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
            <tr><th scope="col">IP</th><th scope="col">Name</th><th scope="col">Description</th></tr>
          </thead>
          <tbody>
            {functions.map(func => (
              <tr key={func.ip}>
                <td><a href={`#/functions/${encodeURIComponent(func.ip)}`}><code>{func.ip}</code></a></td>
                <td><a href={`#/functions/${encodeURIComponent(func.ip)}`}>{func.name ?? 'Unnamed'}</a></td>
                <td>{func.desc ?? 'No description.'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

export function OverviewView() {
  const { data: overview, error } = useJson<Overview>('api/overview.json');

  return (
    <main>
      {error ? (
        <p class="panel" role="alert">Could not load overview: {error}</p>
      ) : overview === null ? (
        <p class="panel" role="status">Loading overview…</p>
      ) : (
        <>
          <OverviewDetails overview={overview} />
          <p><a href="#/memory">View memory map</a></p>
          <FunctionList functions={overview.functions} />
        </>
      )}
    </main>
  );
}
