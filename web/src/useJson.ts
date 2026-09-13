import { useEffect, useState } from 'preact/hooks';

export function useJson<T>(url: string, notFoundMessage?: string) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    setData(null);
    setError(null);

    async function load() {
      try {
        const response = await fetch(url, { signal: controller.signal });
        if (!response.ok) {
          const message = response.status === 404 && notFoundMessage
            ? notFoundMessage
            : `Request failed (${response.status})`;
          throw new Error(message);
        }
        const result: T = await response.json();
        if (!controller.signal.aborted) setData(result);
      } catch (error) {
        if (!controller.signal.aborted) {
          setError(error instanceof Error ? error.message : String(error));
        }
      }
    }

    void load();
    return () => controller.abort();
  }, [url, notFoundMessage]);

  return { data, error };
}
