import App from "./App.svelte";
import "./index.css";

document.addEventListener("contextmenu", (e) => e.preventDefault());

const target = document.getElementById("app");
if (!target) throw new Error("#app not found");

new App({ target });
