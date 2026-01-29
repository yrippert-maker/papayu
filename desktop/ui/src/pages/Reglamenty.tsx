import { useNavigate } from 'react-router-dom';
import { FileText, ArrowLeft } from 'lucide-react';

const SECTIONS = [
  { id: 'armak', name: 'АРМАК' },
  { id: 'faa', name: 'ФАА' },
  { id: 'easa', name: 'ЕАСА' },
  { id: 'mura-menasa', name: 'Mura Menasa' },
];

export function Reglamenty() {
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
            <FileText className="w-8 h-8 text-primary" />
          </div>
          <div>
            <h1 className="text-4xl md:text-5xl font-bold tracking-tight">Регламенты</h1>
            <p className="text-lg text-muted-foreground mt-2">АРМАК, ФАА, ЕАСА, Mura Menasa — разметка документов</p>
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {SECTIONS.map((s) => (
            <div
              key={s.id}
              className="bg-card/80 backdrop-blur-sm border rounded-xl p-6 hover:shadow-lg transition-all-smooth cursor-pointer"
            >
              <div className="font-semibold text-lg">{s.name}</div>
              <p className="text-sm text-muted-foreground mt-1">Документы раздела</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
