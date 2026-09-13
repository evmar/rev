import type { FunctionDetail } from './bindings/FunctionDetail';
import type { XRefDetail } from './bindings/XRefDetail';
import { useJson } from './useJson';

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

export function FunctionView({ ip }: { ip: string }) {
  const url = `api/functions/${encodeURIComponent(ip)}.json`;
  const { data: func, error } = useJson<FunctionDetail>(url, 'Function not found');

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
