export interface RouteConfig {
  path: string;
  name: string;
  component: string;
  description: string;
}

export const ROUTES: Record<string, RouteConfig> = {
  TASKS: { path: '/', name: 'Анализ', component: 'Tasks', description: 'Анализ проекта' },
  CONTROL_PANEL: { path: '/control-panel', name: 'Безопасность', component: 'Dashboard', description: 'Панель безопасности' },
  POLICY_ENGINE: { path: '/policies', name: 'Политики', component: 'PolicyEngine', description: 'Правила безопасности' },
  AUDIT_LOGGER: { path: '/audit', name: 'Аудит', component: 'AuditLogger', description: 'Журнал действий' },
  SECRETS_GUARD: { path: '/secrets', name: 'Секреты', component: 'SecretsGuard', description: 'Защита от утечек' },
  UPDATES: { path: '/updates', name: 'Обновления', component: 'Updates', description: 'Проверка обновлений' },
  DIAGNOSTICS: { path: '/diagnostics', name: 'Диагностика', component: 'Diagnostics', description: 'Версии и логи' },
  LLM_SETTINGS: { path: '/llm-settings', name: 'Настройки LLM', component: 'LlmSettings', description: 'Провайдер, модель, API-ключ' },
};
