import { About } from "./components/About";
import { Setup } from "./components/Setup";
import { Main } from "./components/Main";
import "./setup.css";

export default function App() {
  const view = new URLSearchParams(window.location.search).get("view");
  if (view === "main") return <Main />;
  if (view === "about") return <About />;
  return <Setup />;
}
