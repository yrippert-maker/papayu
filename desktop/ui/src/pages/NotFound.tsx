import { useNavigate } from 'react-router-dom';

export function NotFound() {
  const navigate = useNavigate();
  return (
    <div className="min-h-screen flex items-center justify-center bg-background p-8">
      <div className="max-w-2xl w-full text-center">
        <div className="text-6xl mb-6">404</div>
        <h1 className="text-4xl font-bold mb-4">Страница не найдена</h1>
        <p className="text-lg text-muted-foreground mb-8">Запрашиваемая страница не существует или была перемещена</p>
        <button
          onClick={() => navigate('/')}
          className="px-6 py-3 bg-primary text-primary-foreground rounded-md hover:bg-primary/90 transition-smooth"
        >
          Вернуться на главную
        </button>
      </div>
    </div>
  );
}
