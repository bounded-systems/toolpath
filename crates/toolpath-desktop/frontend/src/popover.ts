import { mount } from "svelte";
import Popover from "./routes/Popover.svelte";
import "./styles.css";

const target = document.getElementById("popover");
if (!target) throw new Error("#popover not found");
mount(Popover, { target });
