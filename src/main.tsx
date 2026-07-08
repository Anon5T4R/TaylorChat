import ReactDOM from "react-dom/client";
import App from "./App";
import "./App.css";

// Sem StrictMode: efeitos de rede/listeners não devem montar em dobro (lição da suíte).
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<App />);
