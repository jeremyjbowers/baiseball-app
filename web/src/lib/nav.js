import { writable } from "svelte/store";

function current() {
  return `${window.location.pathname}${window.location.search}`;
}

export const route = writable(current());

export function navigate(href) {
  const next = href.startsWith("http") ? new URL(href) : new URL(href, window.location.origin);
  const value = `${next.pathname}${next.search}`;
  if (`${window.location.pathname}${window.location.search}` !== value) {
    history.pushState({}, "", value);
  }
  route.set(value);
  window.scrollTo(0, 0);
}

window.addEventListener("popstate", () => {
  route.set(current());
});

export function match(value) {
  const url = new URL(value, window.location.origin);
  const pathname = url.pathname;
  const params = url.searchParams;
  if (pathname === "/") return { name: "sign-in", params };
  if (pathname === "/sign-in/sent") return { name: "sent" };
  if (pathname === "/auth/verify") return { name: "verify", params };
  if (pathname === "/terms" || pathname === "/terms/") return { name: "terms" };
  if (pathname === "/privacy" || pathname === "/privacy/") return { name: "privacy" };
  if (pathname === "/account") return { name: "account" };
  if (pathname === "/leagues") return { name: "leagues" };
  if (pathname === "/leagues/new") return { name: "new-league" };
  let found = pathname.match(/^\/leagues\/([^/]+)\/schedule$/);
  if (found) return { name: "schedule", id: decodeURIComponent(found[1]) };
  found = pathname.match(/^\/leagues\/([^/]+)$/);
  if (found) return { name: "league", id: decodeURIComponent(found[1]) };
  if (pathname === "/teams") return { name: "teams" };
  found = pathname.match(/^\/teams\/([^/]+)\/lineups\/new$/);
  if (found) return { name: "new-lineup", teamId: decodeURIComponent(found[1]) };
  found = pathname.match(/^\/teams\/([^/]+)\/lineups\/([^/]+)$/);
  if (found) {
    return {
      name: "lineup",
      teamId: decodeURIComponent(found[1]),
      lineupId: decodeURIComponent(found[2]),
    };
  }
  found = pathname.match(/^\/teams\/([^/]+)$/);
  if (found) return { name: "team", id: decodeURIComponent(found[1]) };
  return { name: "missing" };
}
