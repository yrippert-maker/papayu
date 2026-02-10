import { useState, useEffect, useCallback } from "react";

type Theme = "light" | "dark";

const STORAGE_KEY = "papa_yu_theme";

function getInitialTheme(): Theme {
  if (typeof window === "undefined") return "light";
  
  // Check localStorage first
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "dark" || stored === "light") {
    return stored;
  }
  
  // Check system preference
  if (window.matchMedia?.("(prefers-color-scheme: dark)").matches) {
    return "dark";
  }
  
  return "light";
}

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(getInitialTheme);

  useEffect(() => {
    // Apply theme to document
    document.documentElement.setAttribute("data-theme", theme);
    localStorage.setItem(STORAGE_KEY, theme);
  }, [theme]);

  useEffect(() => {
    // Listen for system theme changes
    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = (e: MediaQueryListEvent) => {
      const stored = localStorage.getItem(STORAGE_KEY);
      // Only auto-switch if user hasn't explicitly set a preference
      if (!stored) {
        setThemeState(e.matches ? "dark" : "light");
      }
    };

    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, []);

  const toggleTheme = useCallback(() => {
    setThemeState((prev) => (prev === "light" ? "dark" : "light"));
  }, []);

  const setTheme = useCallback((newTheme: Theme) => {
    setThemeState(newTheme);
  }, []);

  return { theme, toggleTheme, setTheme, isDark: theme === "dark" };
}
