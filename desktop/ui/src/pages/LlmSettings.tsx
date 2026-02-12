import { useState, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { Settings as SettingsIcon, ArrowLeft, Save, Eye, EyeOff, Zap, CheckCircle2, XCircle } from 'lucide-react';
import { ROUTES } from '../config/routes';
import { DEFAULT_LLM_SETTINGS, LLM_MODELS, askLlm, type LlmSettings } from '../lib/analyze';

const STORAGE_KEY = 'papayu_llm_settings';

function loadSettings(): LlmSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULT_LLM_SETTINGS, ...JSON.parse(raw) };
  } catch { /* ignored */ }
  return { ...DEFAULT_LLM_SETTINGS };
}

function saveSettings(s: LlmSettings) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
}

export function LlmSettingsPage() {
  const navigate = useNavigate();
  const [settings, setSettings] = useState<LlmSettings>(loadSettings);
  const [showKey, setShowKey] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [saved, setSaved] = useState(false);

  const providerConfig = LLM_MODELS[settings.provider];
  const models = providerConfig?.models ?? [];

  const handleProviderChange = useCallback((provider: string) => {
    const newModels = LLM_MODELS[provider]?.models ?? [];
    setSettings((s) => ({
      ...s,
      provider,
      model: newModels[0]?.value ?? s.model,
    }));
  }, []);

  const handleSave = () => {
    saveSettings(settings);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const resp = await askLlm(
        settings,
        {
          concise_summary: 'Тестовый проект; Node.js; 10 файлов, 3 папки. Риск: Low, зрелость: MVP.',
          key_risks: [],
          top_recommendations: ['Добавить тесты'],
          signals: [],
        },
        'Ответь одним предложением: подключение работает.'
      );
      if (resp.ok) {
        setTestResult({ ok: true, message: `✓ ${resp.content.slice(0, 100)}` });
      } else {
        setTestResult({ ok: false, message: resp.error || 'Неизвестная ошибка' });
      }
    } catch (e) {
      setTestResult({ ok: false, message: String(e) });
    }
    setTesting(false);
  };

  return (
    <div className="max-w-2xl mx-auto p-6 space-y-6">
      <div className="flex items-center gap-3 mb-6">
        <button onClick={() => navigate(ROUTES.TASKS.path)} className="p-2 rounded-lg hover:bg-muted transition-colors">
          <ArrowLeft className="w-5 h-5" />
        </button>
        <SettingsIcon className="w-6 h-6 text-primary" />
        <h1 className="text-xl font-semibold">Настройки LLM</h1>
      </div>

      {/* Provider */}
      <div className="space-y-2">
        <label className="text-sm font-medium text-muted-foreground">Провайдер</label>
        <div className="grid grid-cols-3 gap-2">
          {Object.entries(LLM_MODELS).map(([key, cfg]) => (
            <button
              key={key}
              onClick={() => handleProviderChange(key)}
              className={`px-4 py-2 rounded-lg border text-sm font-medium transition-all ${
                settings.provider === key
                  ? 'border-primary bg-primary/10 text-primary'
                  : 'border-border hover:border-primary/40'
              }`}
            >
              {cfg.label}
            </button>
          ))}
        </div>
      </div>

      {/* Model */}
      <div className="space-y-2">
        <label className="text-sm font-medium text-muted-foreground">Модель</label>
        <select
          value={settings.model}
          onChange={(e) => setSettings((s) => ({ ...s, model: e.target.value }))}
          className="w-full px-3 py-2 rounded-lg border border-border bg-background text-sm"
        >
          {models.map((m) => (
            <option key={m.value} value={m.value}>{m.label}</option>
          ))}
        </select>
      </div>

      {/* API Key */}
      {settings.provider !== 'ollama' && (
        <div className="space-y-2">
          <label className="text-sm font-medium text-muted-foreground">
            API-ключ ({providerConfig?.label})
          </label>
          <div className="relative">
            <input
              type={showKey ? 'text' : 'password'}
              value={settings.apiKey}
              onChange={(e) => setSettings((s) => ({ ...s, apiKey: e.target.value }))}
              placeholder={settings.provider === 'openai' ? 'sk-...' : 'sk-ant-...'}
              className="w-full px-3 py-2 pr-10 rounded-lg border border-border bg-background text-sm font-mono"
            />
            <button
              onClick={() => setShowKey(!showKey)}
              className="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-muted-foreground hover:text-foreground"
            >
              {showKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            </button>
          </div>
          <p className="text-xs text-muted-foreground">
            Ключ хранится локально на вашем устройстве.
          </p>
        </div>
      )}

      {/* Base URL (Ollama or custom) */}
      {settings.provider === 'ollama' && (
        <div className="space-y-2">
          <label className="text-sm font-medium text-muted-foreground">URL Ollama</label>
          <input
            type="text"
            value={settings.baseUrl || 'http://localhost:11434'}
            onChange={(e) => setSettings((s) => ({ ...s, baseUrl: e.target.value }))}
            className="w-full px-3 py-2 rounded-lg border border-border bg-background text-sm font-mono"
          />
        </div>
      )}

      {/* Actions */}
      <div className="flex gap-3 pt-4">
        <button
          onClick={handleSave}
          className="flex items-center gap-2 px-4 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:opacity-90 transition-opacity"
        >
          {saved ? <CheckCircle2 className="w-4 h-4" /> : <Save className="w-4 h-4" />}
          {saved ? 'Сохранено!' : 'Сохранить'}
        </button>
        <button
          onClick={handleTest}
          disabled={testing}
          className="flex items-center gap-2 px-4 py-2 rounded-lg border border-border text-sm font-medium hover:bg-muted transition-colors disabled:opacity-50"
        >
          <Zap className="w-4 h-4" />
          {testing ? 'Проверяю...' : 'Тест подключения'}
        </button>
      </div>

      {/* Test result */}
      {testResult && (
        <div
          className={`p-3 rounded-lg text-sm ${
            testResult.ok ? 'bg-green-500/10 text-green-600 border border-green-500/20' : 'bg-red-500/10 text-red-600 border border-red-500/20'
          }`}
        >
          <div className="flex items-start gap-2">
            {testResult.ok ? <CheckCircle2 className="w-4 h-4 mt-0.5 flex-shrink-0" /> : <XCircle className="w-4 h-4 mt-0.5 flex-shrink-0" />}
            <span>{testResult.message}</span>
          </div>
        </div>
      )}

      {/* Info */}
      <div className="p-4 rounded-lg bg-muted/50 text-xs text-muted-foreground space-y-1">
        <p><strong>OpenAI:</strong> GPT-4o для глубокого анализа, GPT-4o Mini для скорости и экономии.</p>
        <p><strong>Anthropic:</strong> Claude для детального, структурированного аудита.</p>
        <p><strong>Ollama:</strong> Бесплатно, локально, без интернета. Установите Ollama и скачайте модель.</p>
      </div>
    </div>
  );
}
