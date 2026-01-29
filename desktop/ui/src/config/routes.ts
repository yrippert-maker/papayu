export interface RouteConfig {
  path: string;
  name: string;
  component: string;
  description: string;
}

export const ROUTES: Record<string, RouteConfig> = {
  DASHBOARD: { path: '/', name: 'Панель', component: 'Dashboard', description: 'Главная панель' },
  TASKS: { path: '/tasks', name: 'Задачи', component: 'Tasks', description: 'Задачи с вложениями' },
  CONTROL_PANEL: { path: '/control-panel', name: 'Панель управления', component: 'Dashboard', description: 'Панель управления' },
  REGLAMENTY: { path: '/reglamenty', name: 'Регламенты', component: 'Reglamenty', description: 'АРМАК, ФАА, ЕАСА' },
  TMC_ZAKUPKI: { path: '/tmc-zakupki', name: 'ТМЦ и закупки', component: 'TMCZakupki', description: 'ТМЦ и закупки' },
  FINANCES: { path: '/finances', name: 'Финансы', component: 'Finances', description: 'Платежи и отчёты' },
  PERSONNEL: { path: '/personnel', name: 'Персонал', component: 'Personnel', description: 'Сотрудники и учёт' },
  CHAT_AGENT: { path: '/chat', name: 'Чат с агентом', component: 'ChatAgent', description: 'Диалог с ИИ агентом' },
  POLICY_ENGINE: { path: '/policies', name: 'Движок политик', component: 'PolicyEngine', description: 'Правила безопасности' },
  AUDIT_LOGGER: { path: '/audit', name: 'Журнал аудита', component: 'AuditLogger', description: 'Действия в системе' },
  SECRETS_GUARD: { path: '/secrets', name: 'Защита секретов', component: 'SecretsGuard', description: 'Защита от утечек' },
  UPDATES: { path: '/updates', name: 'Обновления', component: 'Updates', description: 'Проверка и установка обновлений' },
  DIAGNOSTICS: { path: '/diagnostics', name: 'Диагностика', component: 'Diagnostics', description: 'Версии, пути, логи' },
};
