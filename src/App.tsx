import { BrowserRouter, Routes, Route, NavLink } from "react-router-dom";
import Tasks from "./pages/Tasks";
import Dashboard from "./pages/Dashboard";

function Layout({ children }: { children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", minHeight: "100vh" }}>
      <header
        style={{
          padding: "14px 24px",
          borderBottom: "1px solid var(--color-border)",
          background: "linear-gradient(135deg, #1e3a5f 0%, #2563eb 100%)",
          boxShadow: "0 2px 8px rgba(37, 99, 235, 0.2)",
          display: "flex",
          alignItems: "center",
          gap: "16px",
        }}
      >
        <img
          src="/logo.png"
          alt="PAPAYU"
          style={{
            height: "44px",
            width: "auto",
            objectFit: "contain",
            filter: "drop-shadow(0 1px 2px rgba(0,0,0,0.2))",
          }}
        />
        <span
          style={{
            fontWeight: 700,
            fontSize: "20px",
            color: "#fff",
            letterSpacing: "-0.02em",
            textShadow: "0 1px 2px rgba(0,0,0,0.15)",
          }}
        >
          PAPA YU
        </span>
        <nav style={{ display: "flex", gap: "6px", marginLeft: "28px" }}>
          <NavLink
            to="/"
            end
            style={({ isActive }) => ({
              padding: "10px 18px",
              borderRadius: "999px",
              fontWeight: 600,
              fontSize: "14px",
              textDecoration: "none",
              color: isActive ? "#1e3a5f" : "rgba(255,255,255,0.9)",
              background: isActive ? "#fff" : "rgba(255,255,255,0.15)",
              transition: "background 0.2s ease, color 0.2s ease",
            })}
          >
            Задачи
          </NavLink>
          <NavLink
            to="/panel"
            style={({ isActive }) => ({
              padding: "10px 18px",
              borderRadius: "999px",
              fontWeight: 600,
              fontSize: "14px",
              textDecoration: "none",
              color: isActive ? "#1e3a5f" : "rgba(255,255,255,0.9)",
              background: isActive ? "#fff" : "rgba(255,255,255,0.15)",
              transition: "background 0.2s ease, color 0.2s ease",
            })}
          >
            Панель управления
          </NavLink>
        </nav>
      </header>
      <main style={{ flex: 1, padding: "24px", overflow: "visible", minHeight: 0 }}>{children}</main>
    </div>
  );
}

export default function App() {
  return (
    <BrowserRouter>
      <Layout>
        <Routes>
          <Route path="/" element={<Tasks />} />
          <Route path="/panel" element={<Dashboard />} />
        </Routes>
      </Layout>
    </BrowserRouter>
  );
}
