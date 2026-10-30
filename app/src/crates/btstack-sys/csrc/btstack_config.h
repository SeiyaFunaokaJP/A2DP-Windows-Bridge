/*
 * btstack_config.h - BTstack compile-time configuration for BlueCaster v2
 *
 * Classic Bluetooth A2DP Source + AVRCP with vendor codecs over a host-side
 * HCI transport (WinUSB via nusb, or H4 over TCP to tools/emu), plus LE
 * central / ISO for the experimental LE Audio (bt-host's le module).
 *
 * Same values as v1 (app/src/btstack_config.h at b819893). LE is compiled in
 * always but stays inert unless LE Audio is enabled: by default the HCI
 * transport hides the controller's LE support from BTstack (btstack-sys
 * src/transport.rs, "LE hidden"), so HCI init sends the commands of a build
 * without LE, and SM, the ATT server and the GATT client are only set up
 * when LE Audio is on (Stack::le_init).
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

#ifndef BTSTACK_CONFIG_H
#define BTSTACK_CONFIG_H

/* Platform features */
#define HAVE_ASSERT
#define HAVE_MALLOC
#define HAVE_POSIX_FILE_IO   /* chipset drivers read firmware files */
#define HAVE_POSIX_TIME

/* Classic Bluetooth (required for A2DP) */
#define ENABLE_CLASSIC

/* Security: Simple Secure Pairing (SSP) for headphone pairing */
#define ENABLE_SOFTWARE_AES128

/* Logging (to the HCI dump: BlueCaster routes it to tracing) */
#define ENABLE_LOG_ERROR
#define ENABLE_LOG_INFO

/* L2CAP Enhanced Retransmission (some A2DP devices use this) */
#define ENABLE_L2CAP_ENHANCED_RETRANSMISSION_MODE

/* HCI ACL buffer size - large enough for A2DP media packets */
#define HCI_ACL_PAYLOAD_SIZE (1691 + 4)
#define HCI_INCOMING_PRE_BUFFER_SIZE 14

/* Link key storage for paired devices */
#define NVM_NUM_DEVICE_DB_ENTRIES  16
#define NVM_NUM_LINK_KEYS          16

/* LE (experimental LE Audio). Compiled in always, but inert unless LE Audio
 * is enabled: only then is the controller's LE support visible to BTstack
 * and are SM, the ATT server and the GATT client initialized. BlueCaster acts as
 * LE central and never advertises; ENABLE_LE_PERIPHERAL is only defined
 * because BTstack's extended advertising code (needed for extended scanning)
 * depends on it. */
#define ENABLE_BLE
#define ENABLE_LE_CENTRAL
#define ENABLE_LE_PERIPHERAL
#define ENABLE_LE_SECURE_CONNECTIONS
#define ENABLE_MICRO_ECC_P256              /* software P-256 (3rd-party/micro-ecc) */
#define ENABLE_LE_DATA_LENGTH_EXTENSION
#define ENABLE_LE_EXTENDED_ADVERTISING     /* extended scanning when the controller has it */
#define ENABLE_LE_ISOCHRONOUS_STREAMS      /* CIG / CIS as central, HCI ISO data */

/* ATT server: a read / write callback may answer "pending". btstack-sys does
 * so only while the application is busy (an ATT request processed from
 * within one of its own calls) and lets BTstack ask again right after. */
#define ENABLE_ATT_DELAYED_RESPONSE

/* LE Audio limits: CIS per CIG (N devices x M sink ASEs) and the ISO SDU
 * reassembly size (2 channels x 155 octets of LC3 48_6, plus headroom) */
#define MAX_NR_CIS                 4
#define HCI_ISO_PAYLOAD_SIZE       400

/* CCC values of bonded GATT clients att_server keeps in the bond TLV, for
 * all clients together (default 20; the oldest go first). The PC's GMCS and
 * GTBS have 23 CCCs: room for about 11 bonded clients. At most 256 (one
 * byte of the TLV tag). */
#define NVN_NUM_GATT_SERVER_CCC    256

/* A2DP explicit config: disable SBC auto-selection, allow vendor codec config.
 * Required for LDAC/aptX - BTstack's auto-mode only handles SBC. */
#define ENABLE_A2DP_EXPLICIT_CONFIG

/* A2DP: 11 stream endpoints (LDAC, aptX HD, aptX, aptX LL, SBC (also SBC XQ), AAC, LHDC V5, FastStream, Opus,
 * Opus 05, aptX Adaptive).
 * The pool is static: an endpoint beyond this count fails to register. */
#define MAX_NR_AVDTP_STREAM_ENDPOINTS  11
#define MAX_NR_AVDTP_CONNECTIONS       1
#define MAX_NR_A2DP_SOURCE_CONNECTIONS 1

#endif /* BTSTACK_CONFIG_H */
