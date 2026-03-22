import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

if (import.meta.env.PROD) {
  document.addEventListener('contextmenu', event => event.preventDefault());
}

// ПРЕДОХРАНИТЕЛЬ
class ErrorBoundary extends React.Component<any, { hasError: boolean, error: any, info: any }> {
  constructor(props: any) { super(props); this.state = { hasError: false, error: null, info: null }; }
  static getDerivedStateFromError(error: any) { return { hasError: true, error }; }
  componentDidCatch(error: any, info: any) { console.error(error, info); }
  render() {
    if (this.state.hasError) {
      return (
        <div className="flex flex-col items-center justify-center h-screen w-screen bg-[#0b110b] text-white p-10 font-mono">
          <h1 className="text-4xl font-bold text-red-500 mb-4">КРИТИЧЕСКАЯ ОШИБКА ИНТЕРФЕЙСА</h1>
          <div className="bg-black/80 p-6 rounded-xl border border-red-500/50 w-full max-w-4xl overflow-auto text-sm text-red-300">
            <p className="font-bold text-base mb-2">{this.state.error?.toString()}</p>
            <pre>{this.state.info?.componentStack}</pre>
          </div>
          <button onClick={() => window.location.reload()} className="mt-8 bg-jm-accent text-black px-8 py-3 rounded-xl font-bold">Перезагрузить</button>
        </div>
      );
    }
    return this.props.children;
  }
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>
);