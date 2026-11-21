import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// `npm run dev` serves the UI with hot reload and forwards the API and SIP WebSocket to a
// locally running fs-webrtc (cargo run / make run) on :8090.
const backend = "http://127.0.0.1:8090";

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": backend,
      "/sip": { target: backend, ws: true },
    },
  },
});
