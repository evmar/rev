import { render } from 'preact';
import { useEffect, useState } from 'preact/hooks';
import type { Overview } from './bindings/Overview';
import type { FunctionDetail } from './bindings/FunctionDetail';
import './style.css';
import type { XRefDetail } from './bindings/XRefDetail';
import type { MemoryDetail } from './bindings/MemoryDetail';

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

function MemoryView() {
  const [memory, setMemory] = useState<MemoryDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();

    async function loadMemory() {
      try {
        const response = await fetch('api/memory.json', {
          signal: controller.signal,
        });
        if (!response.ok) {
          throw new Error(`Request failed (${response.status})`);
        }
        const data: MemoryDetail = await response.json();
        if (!controller.signal.aborted) setMemory(data);
      } catch (error) {
        if (!controller.signal.aborted) {
          setError(error instanceof Error ? error.message : String(error));
        }
      }
    }

    void loadMemory();
    return () => controller.abort();
  }, []);

  return (
    <main>
      <a href="#/">← Project overview</a>
      <h1>Memory</h1>
      {error ? (
        <p class="panel" role="alert">Could not load memory: {error}</p>
      ) : memory === null ? (
        <p class="panel" role="status">Loading memory…</p>
      ) : (
        <section class="panel" aria-labelledby="memory-heading">
          <h2 id="memory-heading">Entries ({memory.entries.length})</h2>
          <table>
            <thead>
              <tr><th scope="col">Address</th><th scope="col">Name</th><th scope="col">Type</th><th scope="col">Description</th></tr>
            </thead>
            <tbody>
              {memory.entries.map(entry => (
                <tr key={entry.addr}>
                  <td><code>{entry.addr}</code></td>
                  <td>{entry.name}</td>
                  <td><code>{entry.typ}</code></td>
                  <td>{entry.desc}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </main>
  );
}

function OverviewView() {
  const [overview, setOverview] = useState<Overview | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();

    async function loadOverview() {
      try {
        const response = await fetch('api/overview.json', {
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

function FunctionRefs({ refs }: { refs: XRefDetail[] }) {
  if (!refs?.length) return null;

  return <>{refs.map(([name, ip], index) => {
    return <span key={index}>
      {index > 0 && ', '}
      {ip ? (
        <a href={`#/functions/${encodeURIComponent(ip)}`}>{name}</a>
      ) : name}
    </span>;
  })}</>;
}

function FunctionView({ ip }: { ip: string }) {
  const [func, setFunction] = useState<FunctionDetail | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    async function loadFunction() {
      try {
        const response = await fetch(`api/functions/${encodeURIComponent(ip)}.json`, {
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
            <p>{func.details}</p>
            <dl>
              {!!func.callers?.length && <>
                <dt>Called by</dt>
                <dd><FunctionRefs refs={func.callers} /></dd>
              </>}
              {!!func.callees?.length && <>
                <dt>Calls</dt>
                <dd><FunctionRefs refs={func.callees} /></dd>
              </>}
              {!!func.params?.length && <>
                <dt>Parameters</dt>
                <dd><ul>{func.params.map((param, index) => (
                  <li key={index}><code>{param.name}: {param.type}</code> — {param.value} {param.desc}</li>
                ))}</ul></dd>
              </>}
              {func.ret && <>
                <dt>Return value</dt>
                <dd><code>{func.ret.name}: {func.ret.type}</code> — {func.ret.value} {func.ret.desc}</dd>
              </>}
            </dl>
          </section>
          <section class="panel">
            {func.blocks.length === 0 ? <p>No instructions found.</p> : func.blocks.map((block, index) => (
              <section key={index}>
                {block.instrs.length === 0 ? <p>No instructions in this block.</p> : (
                  <pre><code>{block.instrs.map((instr) => {
                    const label = instr.label !== null ? `${instr.label}:\n` : '';
                    const comment = instr.comment
                      ? instr.comment.split('\n').map(line => `; ${line}\n`).join('')
                      : '';
                    const [jumpTarget, targetIp] = instr.jmp ?? [];
                    let jumpComment;
                    if (jumpTarget) {
                      if (targetIp) {
                        jumpComment = <a href={`#/functions/${encodeURIComponent(targetIp)}`}>
                          {jumpTarget}
                        </a>;
                      } else {
                        jumpComment = jumpTarget;
                      }
                    }
                    if (jumpComment) jumpComment = <>{' ; '}{jumpComment}</>;
                    return <div key={instr.ip}>
                      {`${comment}${label}${instr.ip} ${instr.text}`}
                      {jumpComment}
                    </div>;
                  })}</code></pre>
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
  if (hash === '#/memory') {
    return <MemoryView />;
  }
  return <OverviewView />;
}

render(<App />, document.getElementById('app')!);
