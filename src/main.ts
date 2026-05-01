import { createApp } from "vue";
import { createPinia } from "pinia";
import { createRouter, createWebHashHistory } from "vue-router";
import App from "./App.vue";
import "./assets/main.css";

const app = createApp(App);

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      component: () => import("./layouts/AppLayout.vue"),
      children: [
        {
          path: "",
          name: "home",
          component: () => import("./views/HomeView.vue"),
        },
      ],
    },
  ],
});

app.use(createPinia());
app.use(router);

// Deshabilitar el context menu por defecto del webview solo en producción
if (import.meta.env.PROD) {
  document.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });
}

app.mount("#app");
