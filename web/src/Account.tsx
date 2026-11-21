import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "./api";
import { Icon } from "./ui";

type Form = "password" | "delete";

/** Account settings: change password, delete account. A modal <dialog>: Esc and the close button dismiss it. */
export function Account({
  username,
  onClose,
  onPasswordChanged,
  onDeleted,
}: {
  username: string;
  onClose: () => void;
  onPasswordChanged: () => void;
  onDeleted: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const [busy, setBusy] = useState<Form | null>(null);
  const [message, setMessage] = useState<{ form: Form; text: string; ok?: boolean } | null>(null);

  useEffect(() => {
    ref.current?.showModal();
  }, []);

  function submit(form: Form, action: (f: Record<string, string>) => Promise<string | void>) {
    return async (e: FormEvent<HTMLFormElement>) => {
      e.preventDefault();
      const el = e.currentTarget;
      setBusy(form);
      setMessage(null);
      try {
        const ok = await action(Object.fromEntries(new FormData(el)) as Record<string, string>);
        if (ok) {
          el.reset();
          setMessage({ form, text: ok, ok: true });
        }
      } catch (err) {
        setMessage({ form, text: err instanceof Error ? err.message : String(err) });
      } finally {
        setBusy(null);
      }
    };
  }

  const changePassword = submit("password", async (f) => {
    if (f.new_password !== f.confirm) throw new Error("The new passwords don't match");
    await api.changePassword({ current_password: f.current_password, new_password: f.new_password });
    onPasswordChanged();
    return "Password changed. Your other devices were signed out.";
  });

  const deleteAccount = submit("delete", async (f) => {
    await api.deleteAccount(f.password);
    onDeleted();
  });

  const note = (form: Form) =>
    message?.form === form && (
      <p className={message.ok ? "success" : "error"} role={message.ok ? "status" : "alert"}>
        {message.text}
      </p>
    );

  return (
    <dialog ref={ref} className="card account" onClose={onClose} aria-labelledby="account-title">
      <header>
        <h2 id="account-title">Account</h2>
        <button className="icon-button subtle" onClick={() => ref.current?.close()} aria-label="Close">
          <Icon name="x" />
        </button>
      </header>

      <form onSubmit={changePassword}>
        <h3>Change password</h3>
        {/* Lets password managers attach the new password to the right account. */}
        <input type="hidden" name="username" autoComplete="username" value={username} />
        <label>
          Current password
          <input name="current_password" type="password" autoComplete="current-password" required maxLength={128} />
        </label>
        <label>
          New password
          <input name="new_password" type="password" autoComplete="new-password" required minLength={8} maxLength={128} />
        </label>
        <label>
          Repeat new password
          <input name="confirm" type="password" autoComplete="new-password" required minLength={8} maxLength={128} />
        </label>
        {note("password")}
        <button className="primary" disabled={!!busy}>
          {busy === "password" ? "Please wait…" : "Change password"}
        </button>
      </form>

      <form onSubmit={deleteAccount} className="danger-zone">
        <h3>Delete account</h3>
        <p className="hint">
          Removes <code>{username}</code>, your contacts and your sessions. This can't be undone.
        </p>
        <label>
          Password
          <input name="password" type="password" autoComplete="current-password" required maxLength={128} />
        </label>
        {note("delete")}
        <button className="primary danger" disabled={!!busy}>
          {busy === "delete" ? "Please wait…" : "Delete my account"}
        </button>
      </form>
    </dialog>
  );
}
