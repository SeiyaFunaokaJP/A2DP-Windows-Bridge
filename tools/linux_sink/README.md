# a2dpwb_sink - measuring receiver on Linux

`a2dpwb_sink.py` turns a Linux PC with an ordinary Bluetooth adapter (the
built-in one is fine, no special driver) into an A2DP sink that A2DPWB can
stream to like to headphones. Nothing is played: every media packet that
arrives over the air is measured, and the statistics go back to A2DPWB over
the network, so the sending and the receiving side can be compared. A2DPWB
finds the receiver on the local network by itself.

```
[PC 1: A2DPWB + USB adapter (BTstack)] --A2DP--> [PC 2: Linux, a2dpwb_sink.py (Bumble)]
              ^                                               |
              +---- LAN: UDP 51201 discovery, TCP 51201 statistics
```

While the tool runs, the adapter is taken from BlueZ and driven directly by
[Bumble](https://github.com/google/bumble) through the kernel's HCI user
channel. That is what makes all codecs possible: BlueZ always accepts media
channels with an L2CAP MTU of 672 bytes, too small for LDAC (libldac
encoders need 679); here the MTU is chosen with `--mtu` (default 1005).
Other Bluetooth devices of that PC stop working until the tool exits, then
BlueZ takes the adapter back.

## What is measured

| | |
|:--|:--|
| Packets, bytes, bitrate | per second and in total |
| Lost / late packets | gaps and steps back in the RTP sequence numbers (not for classic aptX and aptX LL, which carry no RTP header) |
| Jitter | RFC 3550 interarrival jitter against the audio carried |
| Gaps | longest time between two packets |
| Playout buffer model | a sink playing with `--buffer` ms (default 200) of audio: **dropouts** = the buffer would have run dry (audible interruption), **skips** = a burst after a stall would have overfilled it to more than twice its size (audio thrown away). The playout clock follows the sender within +-500 ppm, so clock drift is not counted; lost packets are not concealed and shorten the audio |
| Pauses | the source sent nothing for more than 3 s (A2DPWB capturing a silent PC): counted separately, not as dropouts; the playout model starts over when media comes back |
| Stream errors | RTP timestamp steps, SBC / LDAC frame headers, aptX sync, decode errors |
| RSSI | of the connection, read from the controller every second. For BR/EDR this is relative to the controller's golden receive range (about -60 to -40 dBm): 0 = inside it, e.g. -30 = 30 dB too weak. It is not an absolute level |

Decoding uses the distribution's libraries through ctypes when they are
installed: `libsbc1` (SBC), `libfreeaptx0` (aptX, aptX HD, aptX LL),
`libfdk-aac2` (AAC, in Ubuntu's multiverse). LDAC has no open-source decoder:
its frame headers are checked. Without the libraries the stream is still
measured and only the header checks are done.

## Setup (once)

Copy this folder to the Linux PC (it needs nothing else from the
repository). Ubuntu / Debian, Python 3.11 or later:

```bash
sh setup.sh
```

It installs what is missing of `python3-venv` and the decoder libraries
(`libsbc1`, `libfreeaptx0`, `libfdk-aac2` when the distribution has it; asks
for the sudo password), creates `.venv` and installs `requirements.txt`:
Bumble and its dependencies at the versions pinned for `tools/emu`. Safe to
run again. On other distributions install venv support and, optionally, the
libraries yourself; the script does the rest.

## Running

```bash
sudo sh run.sh [options]
```

(`run.sh` starts `.venv/bin/python a2dpwb_sink.py` with the options given.)

Root is needed for the HCI user channel. The tool is discoverable as
"A2DPWB Sink (&lt;host name&gt;)" and accepts pairing (Just Works); keys are kept
in `a2dpwb_sink_keys.json` next to the script.

On the A2DPWB PC (same network segment - the discovery is a broadcast):

- **GUI** (debug mode): **Debug > Peer Receiver Test...** finds the
  receiver by itself, **Start** streams to its Bluetooth adapter with the
  chosen codec, quality, sample rate and bit depth (no profile needed) and a
  steady test tone as the source (or system audio), and
  the window shows what was sent next to what arrived. The receiver's L2CAP
  MTU and playout buffer can be set there too; they apply from Start.
- **CLI**: `A2DPWB.exe --cli --remote-sink auto` finds the receiver, streams
  to it (unless `-d` names another device) and prints its statistics every
  5 seconds and at the end.

Open UDP and TCP port 51201 on the Linux PC if it has a firewall
(`sudo ufw allow 51201`). A2DPWB can also be given the address directly
(`--remote-sink 192.168.1.20`, or the address field) when broadcasts do not
get through, e.g. across routers.

The tool prints one line per second while streaming and a summary when the
stream ends. Stop it with Ctrl+C.

| Option | Description |
|:-------|:------------|
| `--adapter hci1` | Adapter to use (default: the first one) |
| `--mtu <bytes>` | L2CAP MTU offered for AVDTP (default 1005; 672 = like BlueZ, 679 = smallest LDAC accepts) |
| `--codecs sbc aac aptx aptxhd aptxll ldac` | Codecs to offer (default: all) |
| `--packet-types all\|br` | ACL packet types allowed on the link: all (default) or basic rate only, no EDR |
| `--sbc-max-bitpool <n>` | Highest SBC bitpool offered (default 53, as most headphones) |
| `--buffer <ms>` | Playout buffer of the modelled sink (default 200) |
| `--port <n>`, `--bind <addr>` | Statistics / discovery port and address (default `0.0.0.0:51201`) |
| `--csv <file>` | Append one row of statistics per second |
| `--wav <prefix>` | Write the decoded audio to `<prefix>_<stream>_<codec>.wav` |
| `--capture <file.pklg>` | HCI capture of the whole run, for `a2dpwb_decode --received` |
| `--name <name>`, `--no-discoverable`, `--keys <file>` | Bluetooth name, visibility, pairing key file |
| `--transport <spec>` | A Bumble transport instead of an adapter (testing) |
| `--verbose` | Show Bumble's warnings and debug log |

## Limitations

- The numbers describe what the receiver's controller delivers:
  retransmissions happen inside the controllers and only show as delay and
  gaps.
- Classic aptX and aptX LL carry no sequence numbers, so their lost packets
  only show as missing audio (dropouts).
- The tool is a measuring device, not a speaker: nothing is played.

## Statistics protocol

**Discovery**: a UDP datagram `{"type":"discover","version":1}` to port 51201
(broadcast or unicast) is answered with

```json
{"type":"announce","version":1,"tool":"a2dpwb_sink","host":"lab-pc","port":51201,
 "bt_address":"00:1A:7D:DA:71:13","bt_name":"A2DPWB Sink (lab-pc)",
 "codecs":["LDAC","aptX HD","aptX LL","aptX","AAC","SBC"],"mtu":1005,"buffer_ms":200}
```

**Statistics**: TCP, one JSON object per line. A `hello` line (the fields of
`announce`) on connection, then a `stats` line every second:

```json
{"type":"stats","version":1,"time":1790000000.0,"connected":true,"device":"00:A2:D0:00:00:01",
 "streaming":true,"buffer_target_ms":200,"mtu":1005,"rssi":-52,
 "stream":{"id":3,"codec":"SBC","config":"SBC 48000 Hz, joint stereo, ...","packets":1480,
           "bytes":545000,"lost":0,"late":0,"ts_errors":0,"frame_errors":0,"decode_errors":0,
           "underruns":0,"underrun_ms":0,"overflows":0,"overflow_ms":0,"jitter_ms":6.8,
           "max_gap_ms":24,"buffer_ms":196,"interval":{"packets":100,"bytes":36800,"max_gap_ms":19},
           ...}}
```

Counters in `stream` are cumulative for the stream `id` (a new configuration
by the source starts a new id); `interval` covers the last second.

**Settings**: a client may send `{"type":"configure","mtu":679,"buffer_ms":300,
"codecs":["LDAC","SBC"]}` (any subset of the keys) on the TCP connection. They
apply from the next Bluetooth connection / stream and show up in the next
`stats` lines (`mtu`, `buffer_target_ms`, `codecs`).

## Testing without Bluetooth hardware

`tools/emu/emu_vhci.py` (Linux, root, same virtual environment) creates a
virtual adapter through `/dev/vhci`, linked to a virtual controller that
A2DPWB reaches over TCP; a2dpwb_sink then takes that adapter like a real one:

```bash
sudo .venv/bin/python ../emu/emu_vhci.py [--drop 2] [--stall 400/6]   # Linux, terminal 1
sudo sh run.sh                                                      # Linux, terminal 2
A2DPWB.exe --cli --hci-tcp <linux>:9001 -c ldac --test-tone --duration 20 --remote-sink auto
```

`--drop` loses a share of the media packets, `--stall MS/S` holds them back
for MS milliseconds every S seconds and then delivers them at once, to check
that losses, gaps, dropouts and skips are reported. `--silent-links N` makes
the first N connections go silent right after they come up, to check that
A2DPWB drops such a link and connects again (up to 3 attempts).
