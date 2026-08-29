import React from "react";
import ReactDOM from "react-dom/client";
import { DocsApp } from "./DocsApp";
import "./docs.css";

ReactDOM.createRoot(
  document.getElementById("docs-root") as HTMLElement,
).render(
  <React.StrictMode>
    <DocsApp />
  </React.StrictMode>,
);
