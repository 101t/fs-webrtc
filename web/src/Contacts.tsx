import { useState, type FormEvent } from "react";
import { api, type Contact } from "./api";
import { Avatar, Icon } from "./ui";

type Props = {
  contacts: Contact[];
  canCall: boolean;
  onChange: () => void;
  onCall: (username: string, video: boolean) => void;
};

export function Contacts({ contacts, canCall, onChange, onCall }: Props) {
  const [error, setError] = useState("");
  const [username, setUsername] = useState("");

  async function act(fn: () => Promise<unknown>) {
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
    onChange();
  }

  function add(e: FormEvent) {
    e.preventDefault();
    if (username.trim()) act(() => api.addContact(username).then(() => setUsername("")));
  }

  const incoming = contacts.filter((c) => c.status === "incoming");
  const others = contacts.filter((c) => c.status !== "incoming");

  return (
    <>
      <form className="add-contact" onSubmit={add}>
        <input
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          placeholder="Add someone by username"
          aria-label="Username to add"
          autoCapitalize="none"
          spellCheck={false}
        />
        <button className="icon-button" aria-label="Send contact request" title="Send contact request">
          <Icon name="userPlus" />
        </button>
      </form>
      {error && <p className="error small" role="alert">{error}</p>}

      {incoming.length > 0 && (
        <section aria-labelledby="requests">
          <h2 id="requests">Requests</h2>
          <ul className="contacts">
            {incoming.map((c) => (
              <li key={c.username}>
                <Avatar name={c.display_name} seed={c.username} />
                <div className="who">
                  <strong>{c.display_name}</strong>
                  <span>@{c.username} wants to connect</span>
                </div>
                <button className="icon-button ok" title="Accept" aria-label={`Accept ${c.display_name}`}
                  onClick={() => act(() => api.addContact(c.username))}>
                  <Icon name="check" />
                </button>
                <button className="icon-button" title="Decline" aria-label={`Decline ${c.display_name}`}
                  onClick={() => act(() => api.removeContact(c.username))}>
                  <Icon name="x" />
                </button>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section aria-labelledby="contacts">
        <h2 id="contacts">Contacts</h2>
        {others.length === 0 ? (
          <p className="empty">
            No contacts yet. Ask a friend for their username and add them above. Once they add you back, you can
            call each other.
          </p>
        ) : (
          <ul className="contacts">
            {others.map((c) => (
              <li key={c.username}>
                <span className="avatar-wrap">
                  <Avatar name={c.display_name} seed={c.username} />
                  {c.status === "accepted" && <i className={c.online ? "dot online" : "dot"} />}
                </span>
                <div className="who">
                  <strong>{c.display_name}</strong>
                  <span>{c.status === "outgoing" ? "Request sent" : c.online ? "Online" : `@${c.username}`}</span>
                </div>
                {c.status === "accepted" ? (
                  <>
                    <button className="icon-button dial" disabled={!canCall} title="Voice call"
                      aria-label={`Voice call ${c.display_name}`} onClick={() => onCall(c.username, false)}>
                      <Icon name="phone" />
                    </button>
                    <button className="icon-button dial" disabled={!canCall} title="Video call"
                      aria-label={`Video call ${c.display_name}`} onClick={() => onCall(c.username, true)}>
                      <Icon name="video" />
                    </button>
                  </>
                ) : null}
                <button
                  className="icon-button subtle"
                  title={c.status === "outgoing" ? "Cancel request" : "Remove contact"}
                  aria-label={`${c.status === "outgoing" ? "Cancel request to" : "Remove"} ${c.display_name}`}
                  onClick={() =>
                    (c.status === "outgoing" || confirm(`Remove ${c.display_name}? You won't be able to call each other.`)) &&
                    act(() => api.removeContact(c.username))
                  }
                >
                  <Icon name="x" size={16} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  );
}
