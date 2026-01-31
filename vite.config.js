import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
var __dirname = fileURLToPath(new URL(".", import.meta.url));
export default defineConfig({
    base: "./",
    plugins: [react()],
    clearScreen: false,
    resolve: {
        alias: { "@": path.resolve(__dirname, "src") },
    },
    server: {
        port: 5173,
        strictPort: true,
    },
});
