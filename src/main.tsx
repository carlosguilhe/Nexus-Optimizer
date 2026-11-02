import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ConfirmProvider } from "./confirm";
import { I18nProvider } from "./i18n";
import { SettingsProvider } from "./settings";
import { ToastProvider } from "./toast";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <SettingsProvider>
      <I18nProvider>
        <ToastProvider>
          <ConfirmProvider>
            <App />
          </ConfirmProvider>
        </ToastProvider>
      </I18nProvider>
    </SettingsProvider>
  </React.StrictMode>,
);
