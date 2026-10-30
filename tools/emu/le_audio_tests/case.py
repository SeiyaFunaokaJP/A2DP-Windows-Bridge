"""A test case of run_test.py --le-audio (see __init__.py)."""

import dataclasses

STEREO_TONES = (((1000.0, 0.5),), ((1500.0, 0.5),))   # left 1 kHz, right 1.5 kHz (--test-tone)
MONO_TONES = (((1000.0, 0.25), (1500.0, 0.25)),) * 2  # downmix (L + R) / 2 on both channels


@dataclasses.dataclass
class Case:
    name: str                 # also the output folder: <out>/le-<name>
    layout: str               # emu_le_audio_sink.py --layout
    config: str               # regex for the decoder's "Configuration :" line
    # Expected test tone per WAV channel (left, right): ((freq, amp), ...),
    # or None to skip the tone check
    tones: tuple = STEREO_TONES
    sink_args: list = dataclasses.field(default_factory=list)    # extra emu arguments
    bluecaster_args: list = dataclasses.field(default_factory=list)  # extra BlueCaster arguments
    tags: tuple = ()          # profiles exercised, e.g. ('csip',)
    streams: int = 1          # stream set-ups expected (2 when it is set up again)
    # A link goes down on purpose: the sinks may miss the SDUs in flight then,
    # so sent / received only need to match up to a few SDUs, not exactly
    lossy: bool = False
    duration: int = 0         # seconds of streaming (0: run_test.py --duration)
    # BlueCaster is given only the first sink's address (-d), not the others
    # (--le-device): e.g. CSIP finds the other members of a set itself
    first_address_only: bool = False
    # Underruns allowed (BlueCaster's jitter buffer ran empty while streaming and
    # it sent silence). The silence before the buffer first fills, at the
    # start and after a stream is set up again, is not an underrun.
    max_underruns: int = 0
    # Audio sent (SDU sets without that silence) at least this share of the
    # duration; None: 0.9, or 0.6 for lossy cases (the link is down a while)
    min_audio: float = None
    # BlueCaster must not stream but exit with an error matching this regex
    # (bluecaster.log); nothing else is checked then
    expect_error: str = None
