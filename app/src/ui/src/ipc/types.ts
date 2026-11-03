// The protocol types as they arrive. ts-rs types the u64 fields (ids,
// revisions, times, counters) as bigint, but JSON carries plain numbers and
// JSON.parse makes them numbers; Wire<T> is the type on the wire. The
// values stay far below 2^53.

import type { AdapterInfo as BAdapterInfo } from "./bindings/AdapterInfo";
import type { AdapterState as BAdapterState } from "./bindings/AdapterState";
import type { AudioApp as BAudioApp } from "./bindings/AudioApp";
import type { MixerApp as BMixerApp } from "./bindings/MixerApp";
import type { AudioDevice as BAudioDevice } from "./bindings/AudioDevice";
import type { CaptureState as BCaptureState } from "./bindings/CaptureState";
import type { CodecDetails as BCodecDetails } from "./bindings/CodecDetails";
import type { Device as BDevice } from "./bindings/Device";
import type { DiagBundle as BDiagBundle } from "./bindings/DiagBundle";
import type { ProbeResult as BProbeResult } from "./bindings/ProbeResult";
import type { ScriptCheck as BScriptCheck } from "./bindings/ScriptCheck";
import type { ScriptResult as BScriptResult } from "./bindings/ScriptResult";
import type { DiagInfo as BDiagInfo } from "./bindings/DiagInfo";
import type { ErrorInfo as BErrorInfo } from "./bindings/ErrorInfo";
import type { Event as BEvent } from "./bindings/Event";
import type { FirmwareFile as BFirmwareFile } from "./bindings/FirmwareFile";
import type { FlowStepView as BFlowStepView } from "./bindings/FlowStepView";
import type { FlowView as BFlowView } from "./bindings/FlowView";
import type { IsoCounters as BIsoCounters } from "./bindings/IsoCounters";
import type { LeStreamInfo as BLeStreamInfo } from "./bindings/LeStreamInfo";
import type { LinkCounters as BLinkCounters } from "./bindings/LinkCounters";
import type { LinkSample as BLinkSample } from "./bindings/LinkSample";
import type { LinkTotals as BLinkTotals } from "./bindings/LinkTotals";
import type { LogLine as BLogLine } from "./bindings/LogLine";
import type { Method as BMethod } from "./bindings/Method";
import type { Profile as BProfile } from "./bindings/Profile";
import type { ReceivedStream as BReceivedStream } from "./bindings/ReceivedStream";
import type { DevLog as BDevLog } from "./bindings/DevLog";
import type { DevOverrides as BDevOverrides } from "./bindings/DevOverrides";
import type { ReceiverInfo as BReceiverInfo } from "./bindings/ReceiverInfo";
import type { ReceiverState as BReceiverState } from "./bindings/ReceiverState";
import type { ReceiverStats as BReceiverStats } from "./bindings/ReceiverStats";
import type { ServerMessage as BServerMessage } from "./bindings/ServerMessage";
import type { Settings as BSettings } from "./bindings/Settings";
import type { SettingsApplied as BSettingsApplied } from "./bindings/SettingsApplied";
import type { Snapshot as BSnapshot } from "./bindings/Snapshot";
import type { SourceInfo as BSourceInfo } from "./bindings/SourceInfo";
import type { StreamInfo as BStreamInfo } from "./bindings/StreamInfo";
import type { StreamStatus as BStreamStatus } from "./bindings/StreamStatus";
import type { UpdateState as BUpdateState } from "./bindings/UpdateState";
import type { VerboseState as BVerboseState } from "./bindings/VerboseState";

export type Wire<T> = T extends bigint
  ? number
  : T extends readonly (infer U)[]
    ? Wire<U>[]
    : T extends object
      ? { [K in keyof T]: Wire<T[K]> }
      : T;

export type { AclPacket } from "./bindings/AclPacket";
export type { AdapterStatus } from "./bindings/AdapterStatus";
export type { CrashReport } from "./bindings/CrashReport";
export type { DriverKind } from "./bindings/DriverKind";
export type { LogLevel } from "./bindings/LogLevel";
export type { Msg } from "./bindings/Msg";
export type { ReceiverConnection } from "./bindings/ReceiverConnection";
export type { StreamState } from "./bindings/StreamState";

export type Snapshot = Wire<BSnapshot>;
export type SourceInfo = Wire<BSourceInfo>;
export type AdapterInfo = Wire<BAdapterInfo>;
export type AdapterState = Wire<BAdapterState>;
export type AudioApp = Wire<BAudioApp>;
export type MixerApp = Wire<BMixerApp>;
export type AudioDevice = Wire<BAudioDevice>;
export type CaptureState = Wire<BCaptureState>;
export type CodecDetails = Wire<BCodecDetails>;
export type Device = Wire<BDevice>;
export type DiagBundle = Wire<BDiagBundle>;
export type ProbeResult = Wire<BProbeResult>;
export type ScriptCheck = Wire<BScriptCheck>;
export type ScriptResult = Wire<BScriptResult>;
export type DiagInfo = Wire<BDiagInfo>;
export type ErrorInfo = Wire<BErrorInfo>;
export type Event = Wire<BEvent>;
export type FirmwareFile = Wire<BFirmwareFile>;
export type FlowStepView = Wire<BFlowStepView>;
export type FlowView = Wire<BFlowView>;
export type IsoCounters = Wire<BIsoCounters>;
export type LeStreamInfo = Wire<BLeStreamInfo>;
export type LinkCounters = Wire<BLinkCounters>;
export type LinkSample = Wire<BLinkSample>;
export type LinkTotals = Wire<BLinkTotals>;
export type LogLine = Wire<BLogLine>;
export type Method = Wire<BMethod>;
export type Profile = Wire<BProfile>;
export type ReceivedStream = Wire<BReceivedStream>;
export type DevLog = Wire<BDevLog>;
export type DevOverrides = Wire<BDevOverrides>;
export type ReceiverInfo = Wire<BReceiverInfo>;
export type ReceiverState = Wire<BReceiverState>;
export type ReceiverStats = Wire<BReceiverStats>;
export type ServerMessage = Wire<BServerMessage>;
export type Settings = Wire<BSettings>;
export type SettingsApplied = Wire<BSettingsApplied>;
export type StreamInfo = Wire<BStreamInfo>;
export type StreamStatus = Wire<BStreamStatus>;
export type UpdateState = Wire<BUpdateState>;
export type VerboseState = Wire<BVerboseState>;

export type MethodName = Method["method"];

/** The parameters of a method (undefined: it takes none) */
export type ParamsOf<M extends MethodName> =
  Extract<Method, { method: M }> extends { params: infer P } ? P : undefined;

/** Results as documented on protocol.rs' Method; null for the others */
interface Results {
  "state.snapshot": Snapshot;
  "device.scan": Device[];
  "device.windows_paired": string[];
  "adapter.list": AdapterInfo[];
  "adapter.assign_winusb": AdapterInfo[];
  "adapter.restore_driver": AdapterInfo[];
  "adapter.firmware_files": FirmwareFile[];
  "adapter.download_firmware": AdapterInfo[];
  "adapter.probe": ProbeResult;
  "adapter.check_script": ScriptCheck;
  "adapter.run_script": ScriptResult;
  "audio.devices": AudioDevice[];
  "audio.apps": AudioApp[];
  "audio.mixer": MixerApp[];
  "diag.capture_start": string;
  "diag.capture_stop": CaptureState | null;
  "diag.info": DiagInfo;
  "diag.flow": FlowView;
  "diag.recent_logs": LogLine[];
  "diag.set_verbose": VerboseState | null;
  "diag.crashes_seen": null;
  "diag.mark": number;
  "diag.bundle": DiagBundle;
  "receiver.discover": ReceiverInfo[];
  "receiver.connect": ReceiverState;
  "receiver.status": ReceiverState;
  "app.check_update": UpdateState;
  "settings.set": SettingsApplied;
}

export type ResultOf<M extends MethodName> = M extends keyof Results ? Results[M] : null;

/** request(method) for methods without parameters (or whose parameters
 *  may be left out, as diag.info's), request(method, params) otherwise */
export type ArgsOf<M extends MethodName> =
  ParamsOf<M> extends undefined
    ? []
    : null extends ParamsOf<M>
      ? [params?: ParamsOf<M>]
      : [params: ParamsOf<M>];

/** An error answer of the core, or of the connection (codes "ipc.*") */
export class IpcError extends Error {
  readonly code: string;
  readonly msg: ErrorInfo["message"];
  constructor(info: ErrorInfo) {
    super(info.code);
    this.name = "IpcError";
    this.code = info.code;
    this.msg = info.message;
  }

  static from(e: unknown): IpcError {
    if (e instanceof IpcError) return e;
    if (e && typeof e === "object" && "code" in e && "message" in e) {
      const o = e as { code: unknown; message: unknown };
      if (typeof o.code === "string" && o.message && typeof o.message === "object") {
        return new IpcError(o as ErrorInfo);
      }
    }
    return new IpcError({ code: "ipc.failed", message: { key: "ipc.error.failed", args: [String(e)] } });
  }
}
