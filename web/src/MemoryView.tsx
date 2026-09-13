import type { MemoryEntries } from './bindings/MemoryEntries';
import { useJson } from './useJson';

export function MemoryView() {
  const { data: memory, error } = useJson<MemoryEntries>('api/memory.json');

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
                  <td><a href={`#/memory/${encodeURIComponent(entry.addr)}`}><code>{entry.addr}</code></a></td>
                  <td><a href={`#/memory/${encodeURIComponent(entry.addr)}`}>{entry.name}</a></td>
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
