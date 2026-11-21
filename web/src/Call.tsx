import { useEffect, useRef, useState } from "react";
import type { Call as CallState, Phone } from "./phone";
import { Avatar, Icon } from "./ui";

function useVideo(stream: MediaStream | null) {
  const ref = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.srcObject = stream;
  }, [stream]);
  return ref;
}

function useClock(since?: number) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!since) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [since]);
  if (!since) return "";
  const s = Math.max(0, Math.floor((now - since) / 1000));
  const mm = String(Math.floor(s / 60) % 60).padStart(2, "0");
  const ss = String(s % 60).padStart(2, "0");
  return s >= 3600 ? `${Math.floor(s / 3600)}:${mm}:${ss}` : `${mm}:${ss}`;
}

type Props = { call: CallState; name: string; local: MediaStream | null; remote: MediaStream | null; phone: Phone };

/** The active call: remote video (or avatar), self view and controls. */
export function Call({ call, name, local, remote, phone }: Props) {
  const remoteRef = useVideo(remote);
  const localRef = useVideo(local);
  // Show remote video only once frames arrive; an audio-only peer may still have a silent video track.
  const [remoteVideo, setRemoteVideo] = useState(false);
  const clock = useClock(call.connectedAt);
  const status = call.phase === "connected" ? clock : call.phase === "calling" ? "Calling…" : "Connecting…";
  const selfView = !!local?.getVideoTracks().length;

  return (
    <section className={remoteVideo ? "call has-video" : "call"} aria-label={`Call with ${name}`}>
      <video
        className="remote"
        ref={remoteRef}
        autoPlay
        playsInline
        onResize={(e) => setRemoteVideo(e.currentTarget.videoWidth > 0)}
      />
      {!remoteVideo && (
        <div className="call-peer">
          <Avatar name={name} seed={call.peer} size={112} />
          <h2>{name}</h2>
          <p>{status}</p>
        </div>
      )}
      {remoteVideo && (
        <header className="call-bar">
          <strong>{name}</strong>
          <span>{status}</span>
        </header>
      )}
      {selfView && (
        <video className={call.cameraOff ? "self off" : "self"} ref={localRef} autoPlay playsInline muted />
      )}
      <div className="controls">
        <button
          className={call.muted ? "round active" : "round"}
          onClick={() => phone.toggleMute()}
          aria-pressed={call.muted}
          aria-label={call.muted ? "Unmute" : "Mute"}
          title={call.muted ? "Unmute" : "Mute"}
        >
          <Icon name={call.muted ? "micOff" : "mic"} size={24} />
        </button>
        {selfView && (
          <button
            className={call.cameraOff ? "round active" : "round"}
            onClick={() => phone.toggleCamera()}
            aria-pressed={call.cameraOff}
            aria-label={call.cameraOff ? "Turn camera on" : "Turn camera off"}
            title={call.cameraOff ? "Turn camera on" : "Turn camera off"}
          >
            <Icon name={call.cameraOff ? "videoOff" : "video"} size={24} />
          </button>
        )}
        <button className="round danger" onClick={() => phone.hangup()} aria-label="Hang up" title="Hang up">
          <Icon name="hangup" size={24} />
        </button>
      </div>
    </section>
  );
}

/** Ringing dialog for an incoming call. */
export function Incoming({ call, name, phone }: { call: CallState; name: string; phone: Phone }) {
  return (
    <div className="overlay">
      <div className="card incoming" role="alertdialog" aria-labelledby="incoming-name" aria-describedby="incoming-kind">
        <div className="pulse">
          <Avatar name={name} seed={call.peer} size={96} />
        </div>
        <h2 id="incoming-name">{name}</h2>
        <p id="incoming-kind">{call.video ? "Incoming video call" : "Incoming voice call"}</p>
        <div className="incoming-actions">
          <button className="round danger" onClick={() => phone.hangup()} aria-label="Decline" title="Decline">
            <Icon name="hangup" size={24} />
          </button>
          <button className="round accept" onClick={() => phone.answer(false)} aria-label="Answer with voice" title="Answer with voice"
            autoFocus>
            <Icon name="phone" size={24} />
          </button>
          {call.video && (
            <button className="round accept" onClick={() => phone.answer(true)} aria-label="Answer with video" title="Answer with video">
              <Icon name="video" size={24} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
