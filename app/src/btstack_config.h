/*
 * btstack_config.h - BTstack compile-time configuration for A2DPWB
 *
 * Minimal configuration for Classic Bluetooth A2DP Source
 * with high-quality codecs (LDAC, aptX HD, aptX LL) over WinUSB.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef BTSTACK_CONFIG_H
#define BTSTACK_CONFIG_H

/* Platform features */
#define HAVE_ASSERT
#define HAVE_MALLOC
#define HAVE_POSIX_FILE_IO
#define HAVE_POSIX_TIME

/* Classic Bluetooth (required for A2DP) */
#define ENABLE_CLASSIC

/* Security: Simple Secure Pairing (SSP) for headphone pairing */
#define ENABLE_SOFTWARE_AES128

/* Logging */
#define ENABLE_LOG_ERROR
#define ENABLE_LOG_INFO
#define ENABLE_PRINTF_HEXDUMP

/* L2CAP Enhanced Retransmission (some A2DP devices use this) */
#define ENABLE_L2CAP_ENHANCED_RETRANSMISSION_MODE

/* HCI ACL buffer size - large enough for A2DP media packets */
#define HCI_ACL_PAYLOAD_SIZE (1691 + 4)
#define HCI_INCOMING_PRE_BUFFER_SIZE 14

/* Link key storage for paired devices */
#define NVM_NUM_DEVICE_DB_ENTRIES  16
#define NVM_NUM_LINK_KEYS          16

/* A2DP explicit config: disable SBC auto-selection, allow vendor codec config.
 * Required for LDAC/aptX — BTstack's auto-mode only handles SBC. */
#define ENABLE_A2DP_EXPLICIT_CONFIG

/* A2DP: 5 stream endpoints (LDAC, aptX HD, aptX LL, SBC, AAC) */
#define MAX_NR_AVDTP_STREAM_ENDPOINTS  5
#define MAX_NR_AVDTP_CONNECTIONS       1
#define MAX_NR_A2DP_SOURCE_CONNECTIONS 1

#endif /* BTSTACK_CONFIG_H */
