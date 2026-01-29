import { useNavigate } from 'react-router-dom';
import { FolderOpen, ArrowLeft } from 'lucide-react';

export function Documents() {
  const navigate = useNavigate();

  return (
    <div className="min-h-screen p-8 md:p-12 bg-gradient-to-br from-background via-background to-muted/20">
      <button
        onClick={() => navigate('/')}
        className="mb-8 inline-flex items-center gap-2 text-sm text-muted-foreground hover:text-foreground transition-all-smooth"
      >
        <ArrowLeft className="w-4 h-4" />
        Назад к панели
      </button>
      <div className="animate-fade-in">
        <div className="flex items-center gap-4 mb-8">
          <div className="p-3 rounded-xl bg-primary/10">
            <FolderOpen className="w-8 h-8 text-primary" />
          </div>
          <div>
            <h1 className="text-4xl md:text-5xl font-bold tracking-tight">Документы</h1>
            <p className="text-lg text-muted-foreground mt-2">Все документы компании Mura Menasa</p>
          </div>
        </div>
        <div className="bg-card/80 backdrop-blur-sm border rounded-xl p-6">
          <p className="text-muted-foreground">Раздел документов компании.</p>
        </div>
      </div>
    </div>
  );
}
