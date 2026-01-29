import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = {
    hasError: false,
    error: null,
    errorInfo: null,
  };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    this.setState({ error, errorInfo });
    console.error('ErrorBoundary:', error, errorInfo);
  }

  render() {
    if (this.state.hasError && this.state.error) {
      if (this.props.fallback) return this.props.fallback;
      return (
        <div className="min-h-screen flex items-center justify-center bg-background p-8">
          <div className="max-w-2xl w-full bg-card p-8 rounded-xl border">
            <div className="text-center mb-6">
              <div className="text-6xl mb-4">⚠️</div>
              <h1 className="text-3xl font-bold mb-2">Произошла ошибка</h1>
              <p className="text-muted-foreground">Приложение столкнулось с неожиданной ошибкой</p>
            </div>
            {import.meta.env.DEV && (
              <div className="mt-6 p-4 bg-muted rounded-lg">
                <pre className="text-xs overflow-auto">{this.state.error.toString()}</pre>
              </div>
            )}
            <div className="mt-6 flex gap-4 justify-center">
              <button
                onClick={() => this.setState({ hasError: false, error: null, errorInfo: null })}
                className="px-6 py-2 bg-primary text-primary-foreground rounded-md hover:bg-primary/90"
              >
                Вернуться
              </button>
              <button onClick={() => window.location.reload()} className="px-6 py-2 border rounded-md hover:bg-muted">
                Перезагрузить
              </button>
            </div>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
