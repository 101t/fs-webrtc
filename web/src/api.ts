// Typed client for the fs-webrtc REST API (see /docs on the server).

export type ServerInfo = { version: string; signup: boolean; invite_required: boolean };
export type Session = { token: string; username: string; display_name: string };
export type SipAccount = {
  uri: string;
  domain: string;
  password: string;
  websocket_path: string;
  ice_servers: RTCIceServer[];
};
export type Me = { username: string; display_name: string; sip: SipAccount };
export type Contact = {
  username: string;
  display_name: string;
  status: "accepted" | "outgoing" | "incoming";
  online: boolean;
};

const TOKEN_KEY = "fs-webrtc.token";

export const token = {
  get: () => localStorage.getItem(TOKEN_KEY),
  set: (t: string) => localStorage.setItem(TOKEN_KEY, t),
  clear: () => localStorage.removeItem(TOKEN_KEY),
};

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = {};
  const t = token.get();
  if (t) headers.Authorization = `Bearer ${t}`;
  if (body !== undefined) headers["Content-Type"] = "application/json";
  const res = await fetch(path, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
  if (res.status === 204) return undefined as T;
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new ApiError(res.status, data.message ?? `Request failed (${res.status})`);
  return data as T;
}

export const api = {
  info: () => request<ServerInfo>("GET", "/api/v1/info"),
  register: (b: { username: string; password: string; display_name?: string; invite_code?: string }) =>
    request<Session>("POST", "/api/v1/auth/register", b),
  login: (b: { username: string; password: string }) => request<Session>("POST", "/api/v1/auth/login", b),
  logout: () => request<void>("POST", "/api/v1/auth/logout"),
  me: () => request<Me>("GET", "/api/v1/me"),
  changePassword: (b: { current_password: string; new_password: string }) =>
    request<void>("PUT", "/api/v1/me/password", b),
  deleteAccount: (password: string) => request<void>("DELETE", "/api/v1/me", { password }),
  contacts: () => request<Contact[]>("GET", "/api/v1/contacts"),
  addContact: (username: string) => request<Contact>("POST", "/api/v1/contacts", { username }),
  removeContact: (username: string) =>
    request<void>("DELETE", `/api/v1/contacts/${encodeURIComponent(username)}`),
};
