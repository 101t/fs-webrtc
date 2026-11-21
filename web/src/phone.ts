// One SIP user agent per signed-in user (JsSIP over the server's /sip WebSocket) and at most one call.

import { useEffect, useState, useSyncExternalStore } from "react";
import { UA, WebSocketInterface } from "jssip";
import type { RTCSession } from "jssip/lib/RTCSession";
import type { RTCSessionEvent } from "jssip/lib/UA";
import type { SipAccount } from "./api";
import { playTone, type Tone } from "./tones";

export type Registration = "connecting" | "online" | "offline";

export type Call = {
  peer: string; // username
  incoming: boolean;
  video: boolean; // video was offered (incoming) or our camera is on (outgoing)
  phase: "ringing" | "calling" | "connected";
  connectedAt?: number;
  muted: boolean;
  cameraOff: boolean;
};

export type Notice = { peer?: string; message: string };

export type PhoneState = {
  registration: Registration;
  call: Call | null;
  local: MediaStream | null;
  remote: MediaStream | null;
  notice: Notice | null;
};

const IDLE: PhoneState = { registration: "connecting", call: null, local: null, remote: null, notice: null };

export class Phone {
  state = IDLE;
  private listeners = new Set<() => void>();
  private ua: UA;
  private session: RTCSession | null = null;
  private stream: MediaStream | null = null;
  private stopTone: (() => void) | null = null;
  private pcConfig: RTCConfiguration;
  private onUnload = () => this.ua.stop();

  constructor(private account: SipAccount) {
    this.pcConfig = { iceServers: account.ice_servers };
    const scheme = location.protocol === "https:" ? "wss:" : "ws:";
    this.ua = new UA({
      sockets: [new WebSocketInterface(`${scheme}//${location.host}${account.websocket_path}`)],
      uri: account.uri,
      password: account.password,
      register: true,
      register_expires: 300,
      session_timers: false,
      user_agent: "fs-webrtc",
    });
    this.ua.on("registered", () => this.set({ registration: "online" }));
    this.ua.on("unregistered", () => this.set({ registration: "offline" }));
    this.ua.on("registrationFailed", () => this.set({ registration: "offline" }));
    this.ua.on("disconnected", () => this.set({ registration: "connecting" }));
    this.ua.on("newRTCSession", (e: RTCSessionEvent) => {
      if (String(e.originator) === "remote") this.incoming(e.session, e.request.body ?? "");
    });
    this.ua.start();
    window.addEventListener("beforeunload", this.onUnload);
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  };

  stop() {
    window.removeEventListener("beforeunload", this.onUnload);
    this.finish(null);
    this.ua.stop();
  }

  async dial(peer: string, video: boolean) {
    if (this.session) return;
    const media = await this.media(video);
    if (!media) return;
    this.set({
      local: media,
      notice: null,
      call: { peer, incoming: false, video: hasVideo(media), phase: "calling", muted: false, cameraOff: false },
    });
    const session = this.ua.call(`sip:${peer}@${this.account.domain}`, { mediaStream: media, pcConfig: this.pcConfig });
    this.bind(session);
    this.watch(session.connection);
  }

  async answer(video: boolean) {
    const session = this.session;
    if (!session || this.state.call?.phase !== "ringing") return;
    this.tone(null);
    const media = await this.media(video);
    if (!media) return this.hangup();
    if (this.session !== session) return media.getTracks().forEach((t) => t.stop()); // caller gave up meanwhile
    this.set({ local: media });
    session.answer({ mediaStream: media, pcConfig: this.pcConfig });
    this.watch(session.connection);
  }

  hangup() {
    const session = this.session;
    if (!session) return;
    const declining = this.state.call?.incoming && this.state.call.phase === "ringing";
    try {
      session.terminate(declining ? { status_code: 603, reason_phrase: "Decline" } : undefined);
    } catch {
      this.finish(null); // already ended
    }
  }

  toggleMute() {
    const call = this.state.call;
    if (!call) return;
    this.stream?.getAudioTracks().forEach((t) => (t.enabled = call.muted));
    this.set({ call: { ...call, muted: !call.muted } });
  }

  toggleCamera() {
    const call = this.state.call;
    if (!call) return;
    this.stream?.getVideoTracks().forEach((t) => (t.enabled = call.cameraOff));
    this.set({ call: { ...call, cameraOff: !call.cameraOff } });
  }

  dismissNotice() {
    this.set({ notice: null });
  }

  private incoming(session: RTCSession, sdp: string) {
    if (this.session) {
      session.terminate({ status_code: 486, reason_phrase: "Busy Here" });
      return;
    }
    const peer = session.remote_identity.uri.user;
    const video = /^m=video [1-9]/m.test(sdp);
    this.set({ notice: null, call: { peer, incoming: true, video, phase: "ringing", muted: false, cameraOff: false } });
    this.bind(session);
    this.tone("ring");
  }

  private bind(session: RTCSession) {
    this.session = session;
    const incoming = this.state.call?.incoming ?? false;
    const peer = this.state.call?.peer;
    // JsSIP waits for ICE gathering to finish before sending SDP; an unreachable STUN server
    // would stall that for many seconds. Send what we have shortly after the first candidate.
    let iceTimer = 0;
    session.on("icecandidate", (e) => {
      if (!iceTimer) iceTimer = window.setTimeout(() => e.ready(), 1000);
    });
    session.on("progress", () => {
      if (!incoming) this.tone("ringback");
    });
    session.on("accepted", () => {
      this.tone(null);
      if (this.state.call) this.set({ call: { ...this.state.call, phase: "connected", connectedAt: Date.now() } });
    });
    session.on("ended", () => this.finish(null));
    session.on("failed", (e) => {
      clearTimeout(iceTimer);
      const message = failure(e.cause, incoming);
      this.finish(message ? { peer, message } : null);
    });
  }

  private watch(pc: RTCPeerConnection) {
    const remote = new MediaStream();
    pc.addEventListener("track", (e) => {
      remote.addTrack(e.track);
      this.set({ remote: new MediaStream(remote.getTracks()) });
    });
  }

  private async media(video: boolean): Promise<MediaStream | null> {
    try {
      this.stream = await getMedia(video);
      return this.stream;
    } catch (e) {
      this.set({ notice: { message: mediaError(e) } });
      return null;
    }
  }

  private finish(notice: Notice | null) {
    this.tone(null);
    this.stream?.getTracks().forEach((t) => t.stop());
    this.stream = null;
    this.session = null;
    this.set({ call: null, local: null, remote: null, ...(notice ? { notice } : {}) });
  }

  private tone(kind: Tone | null) {
    this.stopTone?.();
    this.stopTone = kind ? playTone(kind) : null;
  }

  private set(patch: Partial<PhoneState>) {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((l) => l());
  }
}

const hasVideo = (s: MediaStream) => s.getVideoTracks().length > 0;

async function getMedia(video: boolean): Promise<MediaStream> {
  if (!navigator.mediaDevices) throw new Error("insecure");
  try {
    return await navigator.mediaDevices.getUserMedia({
      audio: true,
      video: video ? { width: { ideal: 1280 }, height: { ideal: 720 } } : false,
    });
  } catch (e) {
    // No usable camera: fall back to a voice call rather than failing.
    if (video && e instanceof DOMException && e.name !== "NotAllowedError") {
      return navigator.mediaDevices.getUserMedia({ audio: true });
    }
    throw e;
  }
}

function mediaError(e: unknown): string {
  if (!navigator.mediaDevices) return "Calls need a secure connection. Open this site over https://";
  const name = e instanceof DOMException ? e.name : "";
  if (name === "NotAllowedError") return "Allow microphone access in your browser to make calls";
  if (name === "NotFoundError") return "No microphone found";
  return "Your microphone or camera could not be started";
}

function failure(cause: string, incoming: boolean): string | null {
  if (incoming) return cause === "Canceled" || cause === "Request Timeout" ? "Missed call" : null;
  switch (cause) {
    case "Canceled":
      return null;
    case "Busy":
      return "Busy in another call";
    case "Rejected":
      return "Declined your call";
    case "Not Found":
    case "Unavailable":
    case "Request Timeout":
      return "Not available right now";
    case "No Answer":
      return "No answer";
    case "Connection Error":
      return "Can't reach the server";
    case "User Denied Media Access":
      return "Allow microphone access in your browser to make calls";
    default:
      return `Call failed (${cause})`;
  }
}

const noSubscribe = () => () => {};

/** The phone for `account`, re-created when the account changes, and its live state. */
export function usePhone(account: SipAccount) {
  const [phone, setPhone] = useState<Phone | null>(null);
  useEffect(() => {
    const p = new Phone(account);
    setPhone(p);
    return () => p.stop();
  }, [account]);
  const state = useSyncExternalStore(phone?.subscribe ?? noSubscribe, () => phone?.state ?? IDLE);
  return { phone, ...state };
}
