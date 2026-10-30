// Web pages the UI opens (the same ones v1 opened)

const REPO = "https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge";

export const LINKS = {
  repo: REPO,
  setupGuide: `${REPO}/blob/main/docs/setup.md`,
  /** Realtek firmware: linux-firmware rtl_bt (v1 wx_firmware_dialog.cpp) */
  firmwareRealtek: "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt",
  firmwareIntel: "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/intel",
  /** linux-firmware has no Broadcom .hcd; this collects them from the Windows drivers */
  firmwareBroadcom: "https://github.com/winterheart/broadcom-bt-firmware",
  releases: `${REPO}/releases`,
  issues: `${REPO}/issues/new/choose`,
  sponsor: "https://github.com/sponsors/SeiyaFunaokaJP",
  vbCable: "https://vb-audio.com/Cable/",
} as const;

/** The third-party licences of this version (a development build has no
 *  licenses folder next to it): the release tag's copy, not the main
 *  branch's, which may list other components */
export function thirdPartyUrl(version: string): string {
  return `${REPO}/blob/v${version}/THIRD_PARTY_LICENSES.md`;
}

/** What a bug report is prefilled with (the issue form's field ids) */
export interface BugReport {
  version: string;
  /** "copy diagnostic info" text (addresses masked): the Windows build
   *  comes from it, and as much of it as fits goes into the log field */
  info: string;
  /** "RTL8761BU (2357:0604)" */
  adapter?: string;
  headphones?: string;
  /** Profile.codec */
  codec?: string;
  /** Profile.capture_mode */
  captureMode?: string;
  /** The error the app showed, as the start of "What happened?" */
  whatHappened?: string;
  /** The bundle's file name; none when the report comes from an error */
  bundle?: string;
}

/** The issue form's dropdown options for a profile's codec */
const CODEC_OPTIONS: Record<string, string> = {
  auto: "Auto",
  ldac: "LDAC",
  aptxhd: "aptX HD",
  aptxll: "aptX Low Latency",
  aptx: "aptX",
  lhdcv5: "LHDC V5",
  opus: "Opus",
  aac: "AAC",
  sbcxq: "SBC XQ",
  sbc: "SBC",
  faststream: "FastStream",
};
const CAPTURE_OPTIONS: Record<string, string> = {
  loopback: "System Loopback",
  virtual: "Virtual Device",
  app: "Application",
};
/** Characters of the (encoded) diagnostic info in the URL: GitHub refuses
 *  very long URLs */
const MAX_INFO_CHARS = 6000;

/** A new bug report (.github/ISSUE_TEMPLATE/bug_report.yml) with what the
 *  app knows filled in; the user still describes the problem and attaches
 *  the bundle */
export function bugReportUrl(r: BugReport): string {
  const q = new URLSearchParams({ template: "bug_report.yml", version: r.version, interface: "Window / tray" });
  const os = /OS\s*:\s*Windows build (\d+) \(([^)]+)\)/.exec(r.info);
  if (os) q.set("windows", `Windows build ${os[1]} (${os[2]})`);
  if (r.adapter) q.set("adapter", r.adapter);
  if (r.headphones) q.set("headphones", r.headphones);
  const codec = r.codec && CODEC_OPTIONS[r.codec];
  if (codec) q.set("codec", codec);
  const capture = r.captureMode && CAPTURE_OPTIONS[r.captureMode];
  if (capture) q.set("capture_mode", capture);
  if (r.whatHappened) q.set("what_happened", r.whatHappened);
  if (r.bundle) q.set("bundle", `${r.bundle}\n(drag the file here from the Explorer window BlueCaster opened)`);
  // The field renders as a code block already: without the text's own fence
  let info = r.info.replace(/^```text\n/, "").replace(/\n?```\s*$/, "\n");
  if (encodeURIComponent(info).length > MAX_INFO_CHARS) {
    const cut = "\n... (cut here: the full text is diagnostics.txt in the bundle)";
    while (info.length > 0 && encodeURIComponent(info).length > MAX_INFO_CHARS) info = info.slice(0, Math.floor(info.length * 0.9));
    info = info.slice(0, info.lastIndexOf("\n") + 1 || info.length) + cut;
  }
  q.set("logs", info);
  return `${REPO}/issues/new?${q.toString()}`;
}

/** The firmware download page for an adapter's vendor */
export function firmwareUrl(vendor: string): string {
  const v = vendor.toLowerCase();
  if (v.includes("intel")) return LINKS.firmwareIntel;
  if (v.includes("broadcom")) return LINKS.firmwareBroadcom;
  return LINKS.firmwareRealtek;
}

/** The firmware page for a non-Realtek adapter's own files (v1: Intel's
 *  ibt-* in linux-firmware, Broadcom's .hcd); null for vendors with none */
export function vendorFirmwareUrl(vendor: string): string | null {
  const v = vendor.toLowerCase();
  if (v.includes("intel")) return LINKS.firmwareIntel;
  if (v.includes("broadcom")) return LINKS.firmwareBroadcom;
  return null;
}
