import { mount } from "svelte";
import App from "./App.svelte";
import { navigate } from "./lib/nav.js";
import "./app.css";

document.addEventListener("click", (event) => {
  const link = event.target instanceof Element ? event.target.closest("a") : null;
  if (!link || link.target || link.origin !== window.location.origin) return;
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
  event.preventDefault();
  navigate(`${link.pathname}${link.search}`);
});

mount(App, { target: document.getElementById("app") });
