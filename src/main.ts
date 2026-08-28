import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./assets/main.css";
import { installTechnicalInputDefaults } from "./lib/writingAssistance";

const app = createApp(App);

app.use(createPinia());
installTechnicalInputDefaults();

// Deshabilitar el context menu por defecto del webview solo en producción
if (import.meta.env.PROD) {
  document.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });
}

app.mount("#app");
