import React from "react";
import ReactDOM from "react-dom/client";
// Fonts are bundled with the app: it works fully offline (PLAN §0).
import "@fontsource-variable/newsreader/opsz.css";
import "@fontsource-variable/newsreader/opsz-italic.css";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/400-italic.css";
import "@fontsource/ibm-plex-sans/500.css";
import "@fontsource/ibm-plex-sans/600.css";
import "@fontsource/ibm-plex-sans/700.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/shell.css";
import "./styles/content.css";
import { App } from "./app/App";
import { selfTestIfAsked } from "./app/selftest";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

// `heirloom.exe --selftest <report>` only (the build runs it); a normal start does nothing here.
void selfTestIfAsked();
