import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    VitePWA({
      registerType: "autoUpdate",
      includeAssets: [
        "favicon.svg",
        "icons/apple-touch-icon.png",
        "icons/favicon-16.png",
        "icons/favicon-32.png",
        "icons/favicon-48.png",
      ],
      manifest: {
        name: "ONI — Agentic Development Environment",
        short_name: "ONI",
        description:
          "Dispatch coding agents on your machine, verify their work and merge from anywhere.",
        theme_color: "#0B0B0C",
        background_color: "#0B0B0C",
        display: "standalone",
        start_url: "/",
        scope: "/",
        icons: [
          {
            src: "icons/icon-192.png",
            sizes: "192x192",
            type: "image/png",
          },
          {
            src: "icons/icon-512.png",
            sizes: "512x512",
            type: "image/png",
          },
          {
            src: "icons/icon-1024.png",
            sizes: "1024x1024",
            type: "image/png",
          },
        ],
      },
      workbox: {
        navigateFallback: "/index.html",
        globPatterns: ["**/*.{js,css,html,svg,png,woff2}"],
      },
    }),
  ],
  server: {
    host: true,
    port: 5173,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:7766",
        changeOrigin: false,
      },
    },
  },
  build: {
    target: "es2022",
  },
});
