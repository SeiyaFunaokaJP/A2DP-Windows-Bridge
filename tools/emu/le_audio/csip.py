"""CSIP on the emulated sinks: the sinks are the members of one coordinated set.

--csip-set plaintext|encrypted gives every sink a Coordinated Set
Identification Service (bumble.profiles.csip) with the same SIRK, Coordinated
Set Size = the number of sinks of the layout (or --csip-size), rank = sink
index + 1 (or --csip-ranks) and an unlocked Set Member Lock, and puts a
Resolvable Set Identifier (AD 0x2E) into its advertising data. With
"encrypted" the SIRK characteristic returns the SIRK encrypted with the LTK of
the link (sef, CSIS 4.5).

The CSIS is a primary service here, not included by the CAS (bap.py adds a
plain CAS before this module runs); BlueCaster looks for the CSIS among primary
and included services alike.

--csip-missing: the second sink stops advertising right after power on (a
set member out of range): BlueCaster must stream to the first one alone.

The Set Member Lock follows CSIS 5.3 (LoggingCsis; logged as "CSIS lock:
..."). --csip-lock-deny N[@S]: sink S (default 1) answers the first N lock
requests with Lock Denied, as if another coordinator held it.
--csip-no-lock: no Set Member Lock characteristic (it is optional).
"""

import logging

from bumble import gatt
from bumble.att import ATT_Error
from bumble.core import AdvertisingData
from bumble.profiles import csip

log = logging.getLogger('emu_le_audio')

# Any 16 bytes (over-the-air order); fixed so that runs are reproducible
DEFAULT_SIRK = bytes.fromhex('a2d0b0e0c5150001a2d0b0e0c5150002')

# CSIS application errors (CSIS 1.0.1 table 5.1)
LOCK_DENIED = 0x80
LOCK_RELEASE_NOT_ALLOWED = 0x81
INVALID_LOCK_VALUE = 0x82
LOCK_ALREADY_GRANTED = 0x84


class LoggingCsis(csip.CoordinatedSetIdentificationService):
    """Bumble's CSIS, logging the SIRK reads (for the checks), with the Set
    Member Lock semantics of CSIS 5.3 (Bumble's lock is a plain value): one
    client holds the lock; a lock request while another holds it gets Lock
    Denied; the lock is released when its holder disconnects. deny_locks:
    the first N lock requests are denied as if another coordinator held it."""

    def __init__(self, sink_name, *args, deny_locks=0, **kwargs):
        self.sink_name = sink_name
        self.lock_owner = None
        self.deny_locks = deny_locks
        super().__init__(*args, **kwargs)
        if self.set_member_lock_characteristic is not None:
            self.set_member_lock_characteristic.value = gatt.CharacteristicValue(read=self.on_lock_read,
                                                                                 write=self.on_lock_write)

    async def on_sirk_read(self, connection):
        value = await super().on_sirk_read(connection)
        log.info('%s: CSIS SIRK read (%s)', self.sink_name,
                 'encrypted' if value[0] == csip.SirkType.ENCRYPTED else 'plaintext')
        return value

    def on_lock_read(self, connection):
        return bytes([csip.MemberLock.LOCKED if self.lock_owner else csip.MemberLock.UNLOCKED])

    def on_lock_write(self, connection, value):
        if len(value) != 1 or value[0] not in (csip.MemberLock.UNLOCKED, csip.MemberLock.LOCKED):
            log.info('%s: CSIS lock: invalid value %s', self.sink_name, bytes(value).hex())
            raise ATT_Error(INVALID_LOCK_VALUE)
        if value[0] == csip.MemberLock.LOCKED:
            if self.deny_locks > 0:
                self.deny_locks -= 1
                log.info('%s: CSIS lock: Lock Denied (another coordinator holds it)', self.sink_name)
                raise ATT_Error(LOCK_DENIED)
            if self.lock_owner is connection:
                log.info('%s: CSIS lock: Lock Already Granted', self.sink_name)
                raise ATT_Error(LOCK_ALREADY_GRANTED)
            if self.lock_owner is not None:
                log.info('%s: CSIS lock: Lock Denied', self.sink_name)
                raise ATT_Error(LOCK_DENIED)
            self.lock_owner = connection
            connection.once('disconnection', lambda *_: self.on_owner_gone(connection))
            log.info('%s: CSIS lock: locked by BlueCaster', self.sink_name)
        else:
            if self.lock_owner is not connection:
                log.info('%s: CSIS lock: Lock Release Not Allowed', self.sink_name)
                raise ATT_Error(LOCK_RELEASE_NOT_ALLOWED)
            self.lock_owner = None
            log.info('%s: CSIS lock: released by BlueCaster', self.sink_name)

    def on_owner_gone(self, connection):
        if self.lock_owner is connection:
            self.lock_owner = None
            log.info('%s: CSIS lock: released (its holder disconnected)', self.sink_name)


def add_arguments(parser):
    parser.add_argument('--csip-set', choices=('plaintext', 'encrypted'),
                        help='the sinks form a coordinated set (CSIS with this SIRK type, RSI advertised)')
    parser.add_argument('--csip-size', type=int, default=0,
                        help='Coordinated Set Size (default: the number of sinks of the layout)')
    parser.add_argument('--csip-ranks', help='Set Member Rank of each sink, comma separated (default 1,2,...)')
    parser.add_argument('--csip-missing', action='store_true',
                        help='the second set member does not advertise (out of range)')
    parser.add_argument('--csip-lock-deny', metavar='N[@S]',
                        help='sink S (default 1) denies the first N lock requests (another coordinator)')
    parser.add_argument('--csip-no-lock', action='store_true',
                        help='the CSIS has no Set Member Lock (it is optional)')


def _set_size(args):
    if args.csip_size:
        return args.csip_size
    import __main__  # emu_le_audio_sink.py
    return len(getattr(__main__, 'LAYOUTS', {}).get(args.layout, [None, None]))


def configure_sink(sink, args):
    if not args.csip_set:
        return
    ranks = [int(r) for r in args.csip_ranks.split(',')] if args.csip_ranks else []
    rank = ranks[sink.index] if sink.index < len(ranks) else sink.index + 1
    sirk_type = csip.SirkType.ENCRYPTED if args.csip_set == 'encrypted' else csip.SirkType.PLAINTEXT
    size = _set_size(args)
    deny = 0
    if args.csip_lock_deny:
        count, _, index = args.csip_lock_deny.partition('@')
        if int(index or 1) == sink.index:
            deny = int(count)
    service = LoggingCsis(sink.name, DEFAULT_SIRK, sirk_type, coordinated_set_size=size,
                          set_member_lock=None if args.csip_no_lock else csip.MemberLock.UNLOCKED,
                          set_member_rank=rank, deny_locks=deny)
    sink.device.add_service(service)
    rsi = csip.generate_rsi(DEFAULT_SIRK)
    sink.adv_structures.append((AdvertisingData.RESOLVABLE_SET_IDENTIFIER, rsi))
    sink.state['csip'] = dict(service=service, rank=rank, size=size, rsi=rsi)


async def start(sink, args):
    state = sink.state.get('csip')
    if not state:
        return
    log.info('%s: CSIS member, %s SIRK, rank %d of %d, RSI %s', sink.name, args.csip_set, state['rank'],
             state['size'], state['rsi'].hex())
    if args.csip_missing and sink.index == 1:
        await sink.device.stop_advertising()
        log.info('%s: not advertising (--csip-missing)', sink.name)
