import { render } from 'preact';
import { useEffect, useState } from 'preact/hooks';
import type { Overview } from './bindings/Overview';
import type { FunctionDetail } from './bindings/FunctionDetail';
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
                <td><a href={`#/functions/${encodeURIComponent(func.ip)}`}><code>{func.ip}</code></a></td>
                <td><a href={`#/functions/${encodeURIComponent(func.ip)}`}>{func.name ?? 'Unnamed'}</a></td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

function OverviewView() {
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

function FunctionView({ ip }: { ip: string }) {
  const [func, setFunction] = useState<FunctionDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    async function loadFunction() {
      try {
        const response = await fetch(`/api/functions/${encodeURIComponent(ip)}`, {
          signal: controller.signal,
        });
        if (!response.ok) {
          throw new Error(response.status === 404 ? 'Function not found' : `Request failed (${response.status})`);
        }
        const data: FunctionDetail = await response.json();
        if (!controller.signal.aborted) setFunction(data);
      } catch (error) {
        if (!controller.signal.aborted) {
          setError(error instanceof Error ? error.message : String(error));
        }
      }
    }
    void loadFunction();
    return () => controller.abort();
  }, [ip]);

  return (
    <main>
      <a href="#/">← Project overview</a>
      <h1>{func?.name ?? 'Function'} <code>{ip}</code></h1>
      {error ? <p class="panel" role="alert">{error}</p> : func === null ? (
        <p class="panel" role="status">Loading function…</p>
      ) : (
        <>
          <section class="panel">
            <p>{func.desc ?? 'No description.'}</p>
            <dl>
              <dt>Cross-references</dt>
              <dd>{func.xrefs?.join(', ') || 'None'}</dd>
              <dt>Parameters</dt>
              <dd>{func.params?.length ? <ul>{func.params.map((param, index) => (
                <li key={index}><code>{param.name}: {param.type}</code> — {param.value} {param.desc}</li>
              ))}</ul> : 'None'}</dd>
              <dt>Return value</dt>
              <dd>{func.ret ? <><code>{func.ret.name}: {func.ret.type}</code> — {func.ret.value} {func.ret.desc}</> : 'None'}</dd>
            </dl>
          </section>
          <section class="panel">
            {func.blocks.length === 0 ? <p>No instructions found.</p> : func.blocks.map((block, index) => (
              <section key={index}>
                {block.instrs.length === 0 ? <p>No instructions in this block.</p> : (
                  <pre><code>{block.instrs.map(instr =>
                    `${instr.comment ? instr.comment.split('\n').map(line => `; ${line}\n`).join('') : ''}${instr.ip} ${instr.text}`
                  ).join('\n')}</code></pre>
                )}
              </section>
            ))}
          </section>
        </>
      )}
    </main>
  );
}

function App() {
  const [hash, setHash] = useState(window.location.hash);
  useEffect(() => {
    const onHashChange = () => setHash(window.location.hash);
    window.addEventListener('hashchange', onHashChange);
    return () => window.removeEventListener('hashchange', onHashChange);
  }, []);

  if (hash.startsWith('#/functions/')) {
    let ip: string;
    try {
      ip = decodeURIComponent(hash.slice('#/functions/'.length));
    } catch {
      return <main><a href="#/">Project overview</a><p role="alert">Invalid function address.</p></main>;
    }
    return <FunctionView key={ip} ip={ip} />;
  }
  return <OverviewView />;
}

render(<App />, document.getElementById('app')!);
