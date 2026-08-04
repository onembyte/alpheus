import { useEffect, useState } from "react";

export type ThemeMode = "system" | "light" | "dark";

/**
 * Appearance preference, configured from Settings. "system" tracks the macOS
 * appearance live via prefers-color-scheme; explicit modes pin it. Persisted
 * across launches.
 */
export function useTheme() {
  const [mode, setMode] = useState<ThemeMode>(() => {
    const v = localStorage.getItem("sm-theme");
    return v === "light" || v === "dark" || v === "system" ? v : "system";
  });

  useEffect(() => {
    localStorage.setItem("sm-theme", mode);
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme =
        mode === "system" ? (mq.matches ? "dark" : "light") : mode;
    };
    apply();
    if (mode === "system") {
      mq.addEventListener("change", apply);
      return () => mq.removeEventListener("change", apply);
    }
  }, [mode]);

  return { mode, setMode };
}
