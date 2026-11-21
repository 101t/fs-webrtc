import { useEffect, useState, type FormEvent } from "react";
import { api, token, type ServerInfo } from "./api";
import { Logo } from "./ui";

export function Auth({ onSignedIn }: { onSignedIn: () => void }) {
  const [info, setInfo] = useState<ServerInfo | null>(null);
  const [mode, setMode] = useState<"login" | "register">("login");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.info().then(setInfo, () => setError("Can't reach the server"));
  }, []);

  async function submit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const f = Object.fromEntries(new FormData(e.currentTarget)) as Record<string, string>;
    setBusy(true);
    setError("");
    try {
      const session =
        mode === "login"
          ? await api.login({ username: f.username, password: f.password })
          : await api.register({
              username: f.username,
              password: f.password,
              display_name: f.display_name || undefined,
              invite_code: f.invite_code || undefined,
            });
      token.set(session.token);
      onSignedIn();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setBusy(false);
    }
  }

  const register = mode === "register";
  return (
    <main className="auth">
      <form className="card auth-card" onSubmit={submit}>
        <Logo />
        <p className="tagline">Private voice and video calls with the people you know.</p>

        {info?.signup && (
          <div className="tabs" role="tablist">
            <button type="button" role="tab" aria-selected={!register} onClick={() => setMode("login")}>
              Sign in
            </button>
            <button type="button" role="tab" aria-selected={register} onClick={() => setMode("register")}>
              Create account
            </button>
          </div>
        )}

        <label>
          Username
          <input name="username" autoComplete="username" autoCapitalize="none" spellCheck={false} required
            minLength={3} maxLength={32} pattern="[A-Za-z0-9][A-Za-z0-9._\-]*"
            title="3-32 letters, digits, dots, dashes or underscores" />
        </label>
        {register && (
          <label>
            Display name <small>optional</small>
            <input name="display_name" autoComplete="name" maxLength={64} />
          </label>
        )}
        <label>
          Password
          <input name="password" type="password" autoComplete={register ? "new-password" : "current-password"}
            required minLength={register ? 8 : 1} maxLength={128} />
        </label>
        {register && info?.invite_required && (
          <label>
            Invite code
            <input name="invite_code" required autoComplete="off" />
          </label>
        )}

        {error && <p className="error" role="alert">{error}</p>}
        <button className="primary" disabled={busy}>
          {busy ? "Please wait…" : register ? "Create account" : "Sign in"}
        </button>
        {info && !info.signup && <p className="hint">Accounts on this server are created by its owner.</p>}
      </form>
      <footer className="auth-footer">
        <a href="https://github.com/101t/fs-webrtc" target="_blank" rel="noreferrer">
          fs-webrtc {info?.version}
        </a>{" "}
        · open source
      </footer>
    </main>
  );
}
