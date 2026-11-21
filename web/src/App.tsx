import { useCallback, useEffect, useState } from "react";
import { Account } from "./Account";
import { ApiError, api, token, type Contact, type Me } from "./api";
import { Auth } from "./Auth";
import { Call, Incoming } from "./Call";
import { Contacts } from "./Contacts";
import { usePhone } from "./phone";
import { Avatar, Icon, Logo } from "./ui";

export default function App() {
  const [me, setMe] = useState<Me | null>(null);
  const [loading, setLoading] = useState(() => !!token.get());

  const load = useCallback(() => {
    api
      .me()
      .then(setMe, (e) => e instanceof ApiError && e.status === 401 && token.clear())
      .finally(() => setLoading(false));
  }, []);
  useEffect(() => {
    if (token.get()) load();
  }, [load]);

  const forget = useCallback(() => {
    token.clear();
    setMe(null);
  }, []);
  const signOut = useCallback(() => {
    api.logout().catch(() => {});
    forget();
  }, [forget]);

  if (loading) return <main className="auth" aria-busy="true" />;
  if (!me) return <Auth onSignedIn={load} />;
  // A new password also means a new SIP password: reloading `me` re-registers the phone with it.
  return <Home me={me} onSignOut={signOut} onPasswordChanged={load} onDeleted={forget} />;
}

const REGISTRATION_LABEL = { online: "Ready for calls", connecting: "Connecting…", offline: "Offline" };

function Home({
  me,
  onSignOut,
  onPasswordChanged,
  onDeleted,
}: {
  me: Me;
  onSignOut: () => void;
  onPasswordChanged: () => void;
  onDeleted: () => void;
}) {
  const { phone, registration, call, local, remote, notice } = usePhone(me.sip);
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [settings, setSettings] = useState(false);

  const refresh = useCallback(() => {
    api.contacts().then(setContacts, (e) => e instanceof ApiError && e.status === 401 && onSignOut());
  }, [onSignOut]);
  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 10_000);
    return () => clearInterval(t);
  }, [refresh]);

  const nameOf = (username: string) => contacts.find((c) => c.username === username)?.display_name ?? username;
  const ringing = call?.incoming && call.phase === "ringing";

  useEffect(() => {
    document.title = ringing ? `📞 ${nameOf(call.peer)} is calling` : "fs-webrtc";
  });
  useEffect(() => {
    if (!notice || !phone) return;
    const t = setTimeout(() => phone.dismissNotice(), 6000);
    return () => clearTimeout(t);
  }, [notice, phone]);

  return (
    <div className={call && !ringing ? "home in-call" : "home"}>
      <aside className="sidebar">
        <header className="me">
          <Avatar name={me.display_name} seed={me.username} size={44} />
          <div className="who">
            <strong>{me.display_name}</strong>
            <span className={`status ${registration}`}>{REGISTRATION_LABEL[registration]}</span>
          </div>
          {/* Disabled in a call: a password change re-registers the phone, which would drop it. */}
          <button className="icon-button subtle" onClick={() => setSettings(true)} disabled={!!call}
            title="Account" aria-label="Account">
            <Icon name="user" />
          </button>
          <button className="icon-button subtle" onClick={onSignOut} title="Sign out" aria-label="Sign out">
            <Icon name="logout" />
          </button>
        </header>
        <p className="handle">
          Your username: <code>{me.username}</code>
        </p>
        <Contacts
          contacts={contacts}
          canCall={registration === "online" && !call}
          onChange={refresh}
          onCall={(username, video) => phone?.dial(username, video)}
        />
      </aside>

      <main className="stage">
        {call && !ringing && phone ? (
          <Call call={call} name={nameOf(call.peer)} local={local} remote={remote} phone={phone} />
        ) : (
          <div className="idle">
            <Logo />
            <p>Pick a contact to start a voice or video call.</p>
            <p className="hint">Calls are encrypted (DTLS-SRTP) and relayed by your own server.</p>
          </div>
        )}
      </main>

      {settings && (
        <Account username={me.username} onClose={() => setSettings(false)}
          onPasswordChanged={onPasswordChanged} onDeleted={onDeleted} />
      )}
      {ringing && phone && <Incoming call={call} name={nameOf(call.peer)} phone={phone} />}
      {notice && (
        <div className="toast" role="status" onClick={() => phone?.dismissNotice()}>
          {notice.peer && <strong>{nameOf(notice.peer)}</strong>} {notice.message}
        </div>
      )}
    </div>
  );
}
