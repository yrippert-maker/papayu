import { Link } from "react-router-dom";

const sectionStyle: React.CSSProperties = {
  marginBottom: "24px",
  padding: "20px 24px",
  background: "#fff",
  borderRadius: "var(--radius-lg)",
  border: "1px solid var(--color-border)",
  boxShadow: "var(--shadow-sm)",
};

const headingStyle: React.CSSProperties = {
  marginBottom: "12px",
  fontSize: "16px",
  fontWeight: 700,
  color: "#1e3a5f",
  letterSpacing: "-0.01em",
};

const textStyle: React.CSSProperties = {
  color: "var(--color-text)",
  marginBottom: "8px",
  lineHeight: 1.6,
  fontSize: "14px",
};

const listStyle: React.CSSProperties = {
  margin: "8px 0 0 0",
  paddingLeft: "20px",
  lineHeight: 1.7,
  color: "var(--color-text-muted)",
  fontSize: "13px",
};

export default function Dashboard() {
  return (
    <div
      style={{
        maxWidth: 640,
        margin: "0 auto",
      }}
    >
      <h1 style={{ marginBottom: "24px", fontSize: "24px", fontWeight: 700, color: "#1e3a5f", letterSpacing: "-0.02em" }}>
        Панель управления
      </h1>

      <section style={sectionStyle}>
        <h2 style={headingStyle}>Настройки программы</h2>
        <p style={textStyle}>
          <strong>PAPA YU</strong> — написание программ под ключ, анализ и исправление с улучшениями. Ниже отображаются параметры и подсказки по настройке.
        </p>
      </section>

      <section style={sectionStyle}>
        <h2 style={headingStyle}>Подключение ИИ (LLM)</h2>
        <p style={textStyle}>
          Рекомендации и задачи ИИ работают при заданных переменных окружения. Задайте их в файле <code style={{ background: "#f1f5f9", padding: "2px 6px", borderRadius: "4px", fontSize: "12px" }}>.env</code> или в скрипте запуска:
        </p>
        <ul style={listStyle}>
          <li><strong>PAPAYU_LLM_API_URL</strong> — URL API (например OpenAI или Ollama)</li>
          <li><strong>PAPAYU_LLM_API_KEY</strong> — API-ключ (для OpenAI обязателен)</li>
          <li><strong>PAPAYU_LLM_MODEL</strong> — модель (например gpt-4o-mini, llama3.2)</li>
        </ul>
        <p style={{ ...textStyle, marginTop: "12px", fontSize: "13px", color: "var(--color-text-muted)" }}>
          Запуск с OpenAI: используйте скрипт <code style={{ background: "#f1f5f9", padding: "2px 6px", borderRadius: "4px" }}>start-with-openai.sh</code> или задайте переменные вручную.
        </p>
      </section>

      <section style={sectionStyle}>
        <h2 style={headingStyle}>Поведение по умолчанию</h2>
        <p style={textStyle}>
          Для каждого проекта можно задать:
        </p>
        <ul style={listStyle}>
          <li><strong>Автопроверка</strong> — проверка типов, сборки и тестов после применённых изменений (по умолчанию включена)</li>
          <li><strong>Максимум попыток</strong> агента при автоматическом исправлении (по умолчанию 2)</li>
          <li><strong>Максимум действий</strong> за одну транзакцию (по умолчанию 12)</li>
        </ul>
        <p style={{ ...textStyle, marginTop: "12px", fontSize: "13px", color: "var(--color-text-muted)" }}>
          Эти настройки применяются при работе с проектом во вкладке «Задачи» (профиль проекта).
        </p>
      </section>

      <section style={sectionStyle}>
        <h2 style={headingStyle}>Тренды и рекомендации</h2>
        <p style={textStyle}>
          Раздел «Тренды и рекомендации» в левой панели «Задач» загружает актуальные рекомендации по разработке. Обновление — не реже раза в 30 дней. Кнопка «Обновить тренды» подгружает новые данные.
        </p>
      </section>

      <p style={{ marginTop: "24px" }}>
        <Link
          to="/"
          style={{
            display: "inline-block",
            padding: "12px 20px",
            background: "var(--color-primary)",
            color: "#fff",
            fontWeight: 600,
            borderRadius: "var(--radius-md)",
            textDecoration: "none",
            boxShadow: "0 2px 8px rgba(37, 99, 235, 0.35)",
            transition: "transform 0.2s ease, box-shadow 0.2s ease",
          }}
        >
          Перейти в «Задачи» →
        </Link>
      </p>
    </div>
  );
}
