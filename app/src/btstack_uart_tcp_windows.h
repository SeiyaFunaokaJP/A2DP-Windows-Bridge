/*
 * BTstack UART driver over a TCP socket (Windows) - development / testing only
 * SPDX-License-Identifier: MIT
 */

#ifndef BTSTACK_UART_TCP_WINDOWS_H
#define BTSTACK_UART_TCP_WINDOWS_H

#include "btstack_uart.h"

#ifdef __cplusplus
extern "C" {
#endif

/* UART driver for hci_transport_h4_instance_for_uart(); the UART config's
 * device_name is "host:port" of a virtual controller serving H4 over TCP. */
const btstack_uart_t *btstack_uart_tcp_windows_instance(void);

#ifdef __cplusplus
}
#endif

#endif /* BTSTACK_UART_TCP_WINDOWS_H */
