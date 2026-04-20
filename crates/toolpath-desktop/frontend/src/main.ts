import { mount } from "svelte";
import App from "./app.svelte";
import "./styles.css";

const target = document.getElementById("app");
if (!target) throw new Error("#app not found");
mount(App, { target });
