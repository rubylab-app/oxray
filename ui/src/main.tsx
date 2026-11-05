import { render } from "preact";
import { App } from "./app";

document.documentElement.classList.add(
  "bg-bg",
  "text-[16px]",
  "leading-[24px]",
  "text-text",
);

const root = document.getElementById("app");
if (root) {
  render(<App />, root);
}
