import { navigate } from "./nav.js";
import { clearSession, getSession } from "./session.js";

const API = import.meta.env.VITE_API_URL || "http://127.0.0.1:41742";

export async function api(path, options = {}) {
  const headers = { ...(options.headers || {}) };
  if (options.body !== undefined) headers["content-type"] = "application/json";
  const session = getSession();
  if (session?.token) headers.authorization = `Bearer ${session.token}`;

  let response;
  try {
    response = await fetch(`${API}${path}`, { ...options, headers });
  } catch {
    throw new Error("The office did not respond.");
  }

  const text = await response.text();
  let body = {};
  if (text) {
    try {
      body = JSON.parse(text);
    } catch {
      throw new Error("The office sent a response that could not be read.");
    }
  }

  const signingIn = path === "/auth/magic-links" || (path === "/auth/sessions" && options.method === "POST");
  if (response.status === 401 && !signingIn) {
    clearSession();
    if (window.location.pathname !== "/") navigate("/");
    throw new Error(body.error || "Sign in to continue.");
  }
  if (!response.ok) {
    throw new Error(body.error || "The office could not complete that.");
  }
  return body;
}
