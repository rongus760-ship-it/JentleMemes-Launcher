/** P2P голос через polling сигналов. Сигнал hangup завершает звонок у собеседника. */

const ICE_SERVERS: RTCIceServer[] = [
  { urls: "stun:stun.l.google.com:19302" },
  { urls: "stun:stun1.l.google.com:19302" },
];

function signalsPath(conversationId: string, sessionId: string, after: number) {
  return `/api/v1/conversations/${encodeURIComponent(conversationId)}/calls/${encodeURIComponent(sessionId)}/signals?after=${after}`;
}

export type DmVoiceCallRole = "caller" | "callee";

async function openMicStream(micDeviceId?: string): Promise<MediaStream> {
  const base = { video: false } as const;
  if (micDeviceId) {
    try {
      return await navigator.mediaDevices.getUserMedia({
        ...base,
        audio: { deviceId: { ideal: micDeviceId } },
      });
    } catch {
      /* ideal по старому id часто падает */
    }
  }
  return navigator.mediaDevices.getUserMedia({ ...base, audio: true });
}

/** Только VAD / уровень, без вывода в динамики (иначе слышим себя). */
function setupLocalAnalyserOnly(stream: MediaStream | null, ctx: AudioContext): AnalyserNode | null {
  if (!stream) return null;
  const src = ctx.createMediaStreamSource(stream);
  const an = ctx.createAnalyser();
  an.fftSize = 256;
  an.smoothingTimeConstant = 0.35;
  src.connect(an);
  return an;
}

/**
 * Ветка analyser только для измерения уровня (без destination).
 * Удалённый звук играет через &lt;audio srcObject&gt; — в Tauri/WebKit выход
 * AudioContext.destination часто не слышен, из‑за чего «есть обводка уровня, но тишина».
 * Клон потока для анализатора, чтобы не конкурировать с тем же MediaStream на &lt;audio&gt;.
 */
function setupRemoteAnalyserBranch(stream: MediaStream, ctx: AudioContext): {
  analyser: AnalyserNode;
  disconnect: () => void;
} {
  const src = ctx.createMediaStreamSource(stream);
  const an = ctx.createAnalyser();
  an.fftSize = 256;
  an.smoothingTimeConstant = 0.35;
  src.connect(an);
  return {
    analyser: an,
    disconnect: () => {
      try {
        src.disconnect();
      } catch {
        /* ignore */
      }
      try {
        an.disconnect();
      } catch {
        /* ignore */
      }
    },
  };
}

function analyserLevel(an: AnalyserNode | null): number {
  if (!an) return 0;
  const buf = new Uint8Array(an.frequencyBinCount);
  an.getByteFrequencyData(buf);
  let s = 0;
  for (let i = 0; i < buf.length; i++) s += buf[i];
  const avg = s / buf.length / 255;
  return Math.min(1, avg * 2.2);
}

export function runDmVoiceCall(opts: {
  baseUrl: string;
  token: string;
  conversationId: string;
  sessionId: string;
  role: DmVoiceCallRole;
  micDeviceId?: string;
  onRemoteStream: (stream: MediaStream) => void;
  /** Всегда element: звук собеседника через &lt;audio&gt;; Web Audio только для уровней */
  onPlaybackMode?: (mode: "webaudio" | "element") => void;
  onSetupError?: (message: string) => void;
  onConnectionState?: (state: RTCPeerConnectionState) => void;
  /** Собеседник повесил трубку или сигнал hangup */
  onPeerHangup?: () => void;
  onLevels?: (levels: { local: number; remote: number }) => void;
  onTelemetry?: (t: {
    rttMs: number | null;
    connectionState: string;
    iceConnectionState: string;
  }) => void;
}): {
  hangup: () => Promise<void>;
  setMicMuted: (muted: boolean) => void;
  setAudioOutputDeviceId: (deviceId: string) => Promise<void>;
} {
  const base = opts.baseUrl.replace(/\/$/, "");
  let stopped = false;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let statsTimer: ReturnType<typeof setInterval> | null = null;
  let vadFrame: number | null = null;
  let afterId = 0;
  let pc: RTCPeerConnection | null = null;
  let localStream: MediaStream | null = null;
  const remoteMedia = new MediaStream();
  let micMuted = false;
  let connFailTimer: ReturnType<typeof setTimeout> | null = null;
  let disconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let hadConnected = false;
  let audioCtx: AudioContext | null = null;
  let localAnalyser: AnalyserNode | null = null;
  let remoteAnalyser: AnalyserNode | null = null;
  let remoteAnalyserTeardown: (() => void) | null = null;

  const applyMicMute = () => {
    const on = !micMuted;
    localStream?.getAudioTracks().forEach((t) => {
      t.enabled = on;
    });
    pc?.getSenders().forEach((s) => {
      if (s.track?.kind === "audio") s.track.enabled = on;
    });
  };

  const authFetch = (path: string, init?: RequestInit) => {
    const h = new Headers(init?.headers);
    h.set("Authorization", `Bearer ${opts.token}`);
    return fetch(`${base}${path}`, { ...init, headers: h });
  };

  const postSignal = async (type: string, payload: unknown) => {
    if (stopped) return;
    await authFetch(
      `/api/v1/conversations/${encodeURIComponent(opts.conversationId)}/calls/${encodeURIComponent(opts.sessionId)}/signals`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ type, payload }),
      },
    );
  };

  let remoteDescSet = false;
  const iceQueue: RTCIceCandidateInit[] = [];

  const flushIce = async () => {
    if (!pc) return;
    while (iceQueue.length) {
      const ice = iceQueue.shift()!;
      try {
        await pc.addIceCandidate(ice);
      } catch {
        /* ignore */
      }
    }
  };

  const setRemote = async (desc: RTCSessionDescriptionInit) => {
    if (!pc || stopped) return;
    await pc.setRemoteDescription(new RTCSessionDescription(desc));
    remoteDescSet = true;
    await flushIce();
  };

  const pushIce = (payload: unknown) => {
    if (!payload || typeof payload !== "object") return;
    const o = payload as Record<string, unknown>;
    const cand = o.candidate;
    if (typeof cand !== "string" || !cand) return;
    const ice: RTCIceCandidateInit = {
      candidate: cand,
      sdpMid: typeof o.sdpMid === "string" ? o.sdpMid : undefined,
      sdpMLineIndex: typeof o.sdpMLineIndex === "number" ? o.sdpMLineIndex : undefined,
    };
    if (!remoteDescSet) {
      iceQueue.push(ice);
      return;
    }
    void pc?.addIceCandidate(ice).catch(() => {});
  };

  const notifyPeerHangup = () => {
    if (stopped) return;
    opts.onPeerHangup?.();
  };

  const runLoop = async () => {
    if (stopped) return;
    try {
      const r = await authFetch(signalsPath(opts.conversationId, opts.sessionId, afterId));
      if (!r.ok || stopped) return;
      const j = (await r.json()) as { signals?: { id: number; type: string; payload: unknown }[] };
      const list = j.signals || [];
      for (const s of list) {
        afterId = Math.max(afterId, s.id);
      }
      if (list.some((s) => s.type === "hangup")) {
        notifyPeerHangup();
        return;
      }
      const answers = list.filter((s) => s.type === "answer");
      const offers = list.filter((s) => s.type === "offer");
      const ices = list.filter((s) => s.type === "ice");
      if (opts.role === "caller") {
        const ans = answers[0];
        if (ans && pc && !pc.currentRemoteDescription) {
          const sdp = (ans.payload as { sdp?: string })?.sdp;
          if (typeof sdp === "string") await setRemote({ type: "answer", sdp });
        }
        for (const s of ices) pushIce(s.payload);
      } else {
        const off = offers[0];
        if (off && pc && !pc.currentRemoteDescription) {
          const sdp = (off.payload as { sdp?: string })?.sdp;
          if (typeof sdp === "string") {
            await setRemote({ type: "offer", sdp });
            const answer = await pc.createAnswer();
            await pc.setLocalDescription(answer);
            await postSignal("answer", { sdp: answer.sdp });
          }
        }
        for (const s of ices) pushIce(s.payload);
      }
    } catch {
      /* ignore */
    }
  };

  const pushRemoteToCaller = () => {
    if (audioCtx && remoteMedia.getAudioTracks().length > 0) {
      try {
        remoteAnalyserTeardown?.();
      } catch {
        /* ignore */
      }
      remoteAnalyserTeardown = null;
      remoteAnalyser = null;
      const clone = remoteMedia.clone();
      const chain = setupRemoteAnalyserBranch(clone, audioCtx);
      remoteAnalyser = chain.analyser;
      remoteAnalyserTeardown = () => {
        chain.disconnect();
        clone.getTracks().forEach((t) => {
          try {
            t.stop();
          } catch {
            /* ignore */
          }
        });
      };
    }
    opts.onRemoteStream(remoteMedia);
  };

  void (async () => {
    try {
      if (!navigator.mediaDevices?.getUserMedia) {
        opts.onSetupError?.("Браузер не отдаёт доступ к микрофону (mediaDevices).");
        stopped = true;
        return;
      }
      localStream = await openMicStream(opts.micDeviceId);
      applyMicMute();

      try {
        audioCtx = new AudioContext();
        await audioCtx.resume();
        localAnalyser = setupLocalAnalyserOnly(localStream, audioCtx);
      } catch {
        audioCtx = null;
        localAnalyser = null;
      }
      opts.onPlaybackMode?.("element");

      pc = new RTCPeerConnection({ iceServers: ICE_SERVERS });
      pc.onconnectionstatechange = () => {
        if (!pc) return;
        const st = pc.connectionState;
        const ice = pc.iceConnectionState;
        opts.onTelemetry?.({
          rttMs: null,
          connectionState: st,
          iceConnectionState: ice,
        });
        if (st === "connected") {
          hadConnected = true;
          if (disconnectTimer) {
            clearTimeout(disconnectTimer);
            disconnectTimer = null;
          }
        }
        if (st === "connecting" || st === "connected") {
          if (connFailTimer) {
            clearTimeout(connFailTimer);
            connFailTimer = null;
          }
        }
        if (st === "failed") {
          if (connFailTimer) clearTimeout(connFailTimer);
          connFailTimer = setTimeout(() => {
            if (!pc || pc.connectionState !== "failed") return;
            const iceNow = pc.iceConnectionState;
            if (iceNow === "failed" || iceNow === "disconnected") {
              opts.onConnectionState?.("failed");
            }
          }, 1200);
        }
        if (
          hadConnected &&
          (st === "disconnected" || st === "closed" || st === "failed")
        ) {
          if (disconnectTimer) clearTimeout(disconnectTimer);
          disconnectTimer = setTimeout(() => {
            if (!pc) return;
            const st2 = pc.connectionState;
            if (st2 === "disconnected" || st2 === "closed" || st2 === "failed") {
              notifyPeerHangup();
            }
          }, 1500);
        }
      };

      localStream.getTracks().forEach((t) => pc!.addTrack(t, localStream!));
      pc.onicecandidate = (e) => {
        if (stopped || !e.candidate) return;
        void postSignal("ice", e.candidate.toJSON());
      };
      pc.ontrack = (e) => {
        const t = e.track;
        if (t.kind !== "audio") return;
        if (remoteMedia.getTracks().some((x) => x.id === t.id)) return;
        remoteMedia.addTrack(t);
        t.enabled = true;
        pushRemoteToCaller();
      };

      if (opts.role === "caller") {
        const offer = await pc.createOffer({ offerToReceiveAudio: true });
        await pc.setLocalDescription(offer);
        await postSignal("offer", { sdp: offer.sdp });
      }

      pollTimer = setInterval(() => void runLoop(), 380);
      void runLoop();

      const vadLoop = () => {
        if (stopped) return;
        opts.onLevels?.({
          local: analyserLevel(localAnalyser),
          remote: analyserLevel(remoteAnalyser),
        });
        vadFrame = requestAnimationFrame(vadLoop);
      };
      vadFrame = requestAnimationFrame(vadLoop);

      statsTimer = setInterval(async () => {
        if (!pc || stopped) return;
        try {
          const report = await pc.getStats();
          let rtt: number | null = null;
          report.forEach((entry) => {
            const e = entry as Record<string, unknown>;
            if (entry.type === "remote-inbound-rtp" && typeof e.roundTripTime === "number") {
              const ms = e.roundTripTime * 1000;
              if (!Number.isNaN(ms) && ms > 0) rtt = ms;
            }
            if (entry.type === "candidate-pair") {
              const state = e.state as string | undefined;
              const nominated = e.nominated === true;
              if (
                state === "succeeded" ||
                (state === "in-progress" && nominated) ||
                state === "inprogress"
              ) {
                const v = e.currentRoundTripTime;
                if (typeof v === "number" && !Number.isNaN(v) && v > 0) {
                  const ms = v * 1000;
                  if (rtt == null || ms < rtt) rtt = ms;
                }
              }
            }
          });
          opts.onTelemetry?.({
            rttMs: rtt,
            connectionState: pc.connectionState,
            iceConnectionState: pc.iceConnectionState,
          });
        } catch {
          /* ignore */
        }
      }, 1000);
    } catch (err) {
      stopped = true;
      const msg =
        err instanceof Error
          ? err.message || String(err)
          : typeof err === "string"
            ? err
            : "Не удалось открыть микрофон";
      opts.onSetupError?.(msg);
    }
  })();

  return {
    setAudioOutputDeviceId: async (_deviceId: string) => {
      /* Выход звонка задаётся через &lt;audio setSinkId&gt; в ChatTab */
    },
    setMicMuted: (muted: boolean) => {
      micMuted = muted;
      applyMicMute();
    },
    hangup: async () => {
      if (!stopped) await postSignal("hangup", {});
      stopped = true;
      if (connFailTimer) {
        clearTimeout(connFailTimer);
        connFailTimer = null;
      }
      if (disconnectTimer) {
        clearTimeout(disconnectTimer);
        disconnectTimer = null;
      }
      if (vadFrame != null) {
        cancelAnimationFrame(vadFrame);
        vadFrame = null;
      }
      if (statsTimer) {
        clearInterval(statsTimer);
        statsTimer = null;
      }
      if (pollTimer) {
        clearInterval(pollTimer);
        pollTimer = null;
      }
      try {
        if (pc) pc.onconnectionstatechange = null;
      } catch {
        /* ignore */
      }
      try {
        remoteAnalyserTeardown?.();
      } catch {
        /* ignore */
      }
      remoteAnalyserTeardown = null;
      try {
        localAnalyser?.disconnect();
      } catch {
        /* ignore */
      }
      localAnalyser = null;
      remoteAnalyser = null;
      try {
        await audioCtx?.close();
      } catch {
        /* ignore */
      }
      audioCtx = null;
      try {
        pc?.getSenders().forEach((s) => s.track?.stop());
      } catch {
        /* ignore */
      }
      localStream?.getTracks().forEach((t) => t.stop());
      remoteMedia.getTracks().forEach((t) => {
        try {
          t.stop();
        } catch {
          /* ignore */
        }
      });
      try {
        pc?.close();
      } catch {
        /* ignore */
      }
      pc = null;
      localStream = null;
    },
  };
}
