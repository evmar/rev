import { render } from 'preact';
import { useEffect, useState } from 'preact/hooks';
import './style.css';
import { FunctionView } from './FunctionView';
import { MemoryView } from './MemoryView';
import { OverviewView } from './OverviewView';

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
