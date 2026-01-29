import { useEffect } from 'react';
import { useAppStore } from '../store/app-store';

export function ErrorDisplay() {
  const error = useAppStore((s) => s.error);
  const setError = useAppStore((s) => s.setError);

  useEffect(() => {
    if (error) {
      const t = setTimeout(() => setError(null), 10000);
      return () => clearTimeout(t);
    }
  }, [error, setError]);

  if (!error) return null;

  return (
    <div className="fixed top-4 right-4 z-50 max-w-md animate-fade-in">
      <div className="bg-destructive text-destructive-foreground p-4 rounded-lg shadow-lg border border-destructive/50">
        <div className="flex items-start gap-3">
          <span className="text-xl">⚠️</span>
          <div className="flex-1">
            <h3 className="font-semibold mb-1">Ошибка</h3>
            <p className="text-sm">{error}</p>
          </div>
          <button onClick={() => setError(null)} className="hover:opacity-80" aria-label="Закрыть">
            ✕
          </button>
        </div>
      </div>
    </div>
  );
}
