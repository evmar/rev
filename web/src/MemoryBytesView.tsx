import { useState } from 'preact/hooks';
import { useJson } from './useJson';

const PAGE_SIZE = 4096;
const PAGE_SEGMENTS = PAGE_SIZE / 16;

function hex(value: number, width: number) {
  return value.toString(16).padStart(width, '0');
}

export function MemoryBytesView({ seg }: { seg: number }) {
  const segofs = `${hex(seg, 4)}:0000`;
  const [input, setInput] = useState(hex(seg, 4));
  const { data, error } = useJson<number[]>(`api/bytes/${encodeURIComponent(segofs)}.json`);

  const rows: string[] = [];
  if (data) {
    for (let i = 0; i < data.length; i += 16) {
      const bytes = data.slice(i, i + 16);
      const label = `${hex(seg, 4)}:${hex(i, 4)}`;
      const left = bytes.slice(0, 8).map(byte => hex(byte, 2)).join(' ').padEnd(23);
      const right = bytes.slice(8).map(byte => hex(byte, 2)).join(' ').padEnd(23);
      const ascii = bytes.map(byte => byte >= 32 && byte <= 126 ? String.fromCharCode(byte) : '.').join('');
      rows.push(`${label}  ${left}  ${right}  |${ascii.padEnd(16)}|`);
    }
  }

  return (
    <main class="memory-bytes">
      <a href="#/">← Project overview</a>
      <h1>Raw memory</h1>
      <form class="memory-controls" onSubmit={event => {
        event.preventDefault();
        window.location.hash = `/bytes/${encodeURIComponent(input.trim())}`;
      }}>
        <label>Segment <input value={input} onInput={event => setInput(event.currentTarget.value)}
          placeholder="0823" pattern="[0-9a-fA-F]{1,4}" required /></label>
        <button type="submit">Go</button>
      </form>
      {error ? <p class="panel">{error}</p> : data === null ? (
        <p class="panel">Loading memory bytes…</p>
      ) : (
        <>
          <nav class="memory-controls" aria-label="Memory pages">
            {seg > 0 && <a href={`#/bytes/${hex(Math.max(0, seg - PAGE_SEGMENTS), 4)}`}>← Previous 4 KB</a>}
            <span>{data.length.toLocaleString()} bytes at <code>{segofs}</code></span>
            {data.length === PAGE_SIZE && seg + PAGE_SEGMENTS <= 0xffff && (
              <a href={`#/bytes/${hex(seg + PAGE_SEGMENTS, 4)}`}>Next 4 KB →</a>
            )}
          </nav>
          {data.length === 0 ? <p class="panel">No memory bytes at this address.</p> : (
            <pre class="panel hex-dump">{rows.join('\n')}</pre>
          )}
        </>
      )}
    </main>
  );
}
