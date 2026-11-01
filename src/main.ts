import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { inTauri } from "./lib/api";

if (inTauri) document.body.classList.add("tauri");

const app = mount(App, { target: document.getElementById("app")! });

export default app;
