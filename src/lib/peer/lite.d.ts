import { Duplex } from "streamx";
import type { RTCPeerConnection, RTCSessionDescription, RTCIceCandidate } from "webrtc-polyfill";

export interface PeerOptions {
  initiator?: boolean;
  channelName?: string;
  channelConfig?: RTCDataChannelInit;
  config?: RTCConfiguration & { sdpSemantics: string };
  offerOptions?: RTCOfferOptions;
  answerOptions?: RTCAnswerOptions;
  sdpTransform?: (sdp: string) => string;
  trickle?: boolean;
  allowHalfTrickle?: boolean;
  iceCompleteTimeout?: number;
  objectMode?: boolean;
}

export interface PeerSignalData {
  type: string;
  sdp?: string;
  candidate?: RTCIceCandidate;
  transceiverRequest?: {
    kind: string;
    init: RTCRtpTransceiverInit;
  };
  renegotiate?: boolean;
}

export class Peer extends Duplex {
  static WEBRTC_SUPPORT: boolean;
  static config: RTCConfiguration;
  static channelConfig: RTCDataChannelInit;

  constructor(opts?: PeerOptions);
  readonly _channel: RTCDataChannel;
  readonly bufferSize: number;
  readonly connected: boolean;
  readonly destroyed: boolean;
  readonly initiator: boolean;
  readonly channelName: string;

  readonly localAddress: string | undefined;
  readonly localPort: number | undefined;
  readonly remoteAddress: string | undefined;
  readonly remotePort: number | undefined;
  readonly remoteFamily: string | undefined;

  address(): {
    port: number | undefined;
    family: string | undefined;
    address: string | undefined;
  };
  signal(data: string | PeerSignalData): void;
  send(chunk: ArrayBufferView | ArrayBuffer | string | Blob): void;
  addTransceiver(kind: string, init?: RTCRtpTransceiverInit): void;
  getStats(cb: (err: Error | null, stats: RTCStatsReport[]) => void): void;

  on(event: "signal", listener: (data: PeerSignalData) => void): this;
  on(event: "connect", listener: () => void): this;
  on(event: "data", listener: (data: ArrayBuffer | string | Blob) => void): this;
  on(event: "stream", listener: (stream: MediaStream) => void): this;
  on(event: "track", listener: (track: MediaStreamTrack, stream: MediaStream) => void): this;
  on(event: "close", listener: () => void): this;
  on(event: "error", listener: (err: Error) => void): this;
  on(event: "end", listener: () => void): this;
  on(event: "pause", listener: () => void): this;
  on(event: "resume", listener: () => void): this;
  on(
    event: "iceStateChange",
    listener: (iceConnectionState: RTCIceConnectionState, iceGatheringState: RTCIceGatheringState) => void,
  ): this;
  on(event: "signalingStateChange", listener: (signalingState: RTCSignalingState) => void): this;
  on(event: string, listener: (...args: any[]) => void): this;

  destroy(err?: Error): void;
}

export = Peer;
