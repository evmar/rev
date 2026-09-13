import type { MemoryAddressDetail } from './bindings/MemoryAddressDetail';
import { useJson } from './useJson';

export function MemoryAddressView({ addr }: { addr: string }) {
  const url = `api/memory/${encodeURIComponent(addr)}.json`;
  const { data: memory, error } = useJson<MemoryAddressDetail>(url, 'Memory address not found');

  return (
    <main>
      <a href="#/memory">← Memory map</a>
      <h1>{memory?.name ?? 'Memory'} <code>{addr}</code></h1>
      {error ? <p class="panel">{error}</p> : memory === null ? (
        <p class="panel">Loading memory address…</p>
      ) : (
        <>
          <dl class="panel">
            <dt>Type</dt>
            <dd><code>{memory.typ}</code></dd>
            <dt>Description</dt>
            <dd>{memory.desc}</dd>
          </dl>
          <section class="panel">
            <h2>Used in ({memory.functions.length})</h2>
            {memory.functions.length === 0 ? (
              <p>No functions reference this address.</p>
            ) : (
              <table>
                <thead>
                  <tr><th scope="col">IP</th><th scope="col">Name</th><th scope="col">Description</th></tr>
                </thead>
                <tbody>
                  {memory.functions.map(func => (
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
        </>
      )}
    </main>
  );
}
