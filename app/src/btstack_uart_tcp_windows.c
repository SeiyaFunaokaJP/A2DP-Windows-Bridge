/*
 * BTstack UART driver over a TCP socket (Windows)
 *
 * Lets BTstack's H4 transport talk to a virtual Bluetooth controller that
 * exposes H4 over TCP (tools/emu: Bumble virtual controller). Development /
 * testing only: selected with the CLI option --hci-tcp <host:port>.
 *
 * Mirrors the structure of BTstack's Windows serial driver: overlapped
 * WSARecv / WSASend whose event handles are BTstack run loop data sources,
 * so all callbacks run on the BTstack thread.
 *
 * SPDX-License-Identifier: MIT
 */

#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>

#include <stdio.h>
#include <string.h>

#include "btstack_uart_tcp_windows.h"
#include "btstack_run_loop.h"

static char tcp_host[128];
static char tcp_port[16];
static SOCKET tcp_socket = INVALID_SOCKET;

static WSAOVERLAPPED overlapped_read;
static WSAOVERLAPPED overlapped_write;
static btstack_data_source_t data_source_read;
static btstack_data_source_t data_source_write;

static void (*block_received)(void);
static void (*block_sent)(void);

static uint8_t *read_buffer;
static uint16_t read_len;
static uint16_t read_pos;
static const uint8_t *write_buffer;
static uint16_t write_len;
static uint16_t write_pos;

static void start_read(void) {
    WSABUF buf;
    DWORD received = 0, flags = 0;
    buf.buf = (char *)(read_buffer + read_pos);
    buf.len = (ULONG)(read_len - read_pos);
    WSAResetEvent(overlapped_read.hEvent);
    if (WSARecv(tcp_socket, &buf, 1, &received, &flags, &overlapped_read, NULL) != 0 &&
        WSAGetLastError() != WSA_IO_PENDING) {
        fprintf(stderr, "HCI TCP: WSARecv failed (%d)\n", WSAGetLastError());
        return;
    }
    /* Completion (immediate or not) signals the event */
    btstack_run_loop_enable_data_source_callbacks(&data_source_read, DATA_SOURCE_CALLBACK_READ);
}

static void start_write(void) {
    WSABUF buf;
    DWORD sent = 0;
    buf.buf = (char *)(write_buffer + write_pos);
    buf.len = (ULONG)(write_len - write_pos);
    WSAResetEvent(overlapped_write.hEvent);
    if (WSASend(tcp_socket, &buf, 1, &sent, 0, &overlapped_write, NULL) != 0 &&
        WSAGetLastError() != WSA_IO_PENDING) {
        fprintf(stderr, "HCI TCP: WSASend failed (%d)\n", WSAGetLastError());
        return;
    }
    btstack_run_loop_enable_data_source_callbacks(&data_source_write, DATA_SOURCE_CALLBACK_WRITE);
}

static void process_read(btstack_data_source_t *ds, btstack_data_source_callback_type_t type) {
    DWORD received = 0, flags = 0;
    (void)type;
    btstack_run_loop_disable_data_source_callbacks(ds, DATA_SOURCE_CALLBACK_READ);
    if (!WSAGetOverlappedResult(tcp_socket, &overlapped_read, &received, FALSE, &flags)) {
        if (WSAGetLastError() == WSA_IO_INCOMPLETE) {
            btstack_run_loop_enable_data_source_callbacks(ds, DATA_SOURCE_CALLBACK_READ);
        } else {
            fprintf(stderr, "HCI TCP: receive failed (%d)\n", WSAGetLastError());
        }
        return;
    }
    if (received == 0) {
        fprintf(stderr, "HCI TCP: connection closed by the virtual controller\n");
        return;
    }
    read_pos = (uint16_t)(read_pos + received);
    if (read_pos < read_len) {
        start_read();
        return;
    }
    if (block_received) block_received();
}

static void process_write(btstack_data_source_t *ds, btstack_data_source_callback_type_t type) {
    DWORD sent = 0, flags = 0;
    (void)type;
    btstack_run_loop_disable_data_source_callbacks(ds, DATA_SOURCE_CALLBACK_WRITE);
    if (!WSAGetOverlappedResult(tcp_socket, &overlapped_write, &sent, FALSE, &flags)) {
        if (WSAGetLastError() == WSA_IO_INCOMPLETE) {
            btstack_run_loop_enable_data_source_callbacks(ds, DATA_SOURCE_CALLBACK_WRITE);
        } else {
            fprintf(stderr, "HCI TCP: send failed (%d)\n", WSAGetLastError());
        }
        return;
    }
    write_pos = (uint16_t)(write_pos + sent);
    if (write_pos < write_len) {
        start_write();
        return;
    }
    if (block_sent) block_sent();
}

/* device_name: "host:port" */
static int uart_init(const btstack_uart_config_t *config) {
    const char *name = config->device_name ? config->device_name : "";
    const char *colon = strrchr(name, ':');
    size_t host_len = colon ? (size_t)(colon - name) : 0;
    if (!colon || host_len == 0 || host_len >= sizeof(tcp_host) || strlen(colon + 1) >= sizeof(tcp_port)) {
        fprintf(stderr, "HCI TCP: expected host:port, got '%s'\n", name);
        return -1;
    }
    memcpy(tcp_host, name, host_len);
    tcp_host[host_len] = '\0';
    strcpy(tcp_port, colon + 1);
    return 0;
}

static int uart_open(void) {
    WSADATA wsa;
    struct addrinfo hints, *res = NULL, *ai;
    BOOL nodelay = TRUE;

    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) return -1;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_protocol = IPPROTO_TCP;
    if (getaddrinfo(tcp_host, tcp_port, &hints, &res) != 0) {
        fprintf(stderr, "HCI TCP: cannot resolve %s:%s\n", tcp_host, tcp_port);
        return -1;
    }
    for (ai = res; ai; ai = ai->ai_next) {
        tcp_socket = WSASocketW(ai->ai_family, ai->ai_socktype, ai->ai_protocol, NULL, 0,
                                WSA_FLAG_OVERLAPPED);
        if (tcp_socket == INVALID_SOCKET) continue;
        if (connect(tcp_socket, ai->ai_addr, (int)ai->ai_addrlen) == 0) break;
        closesocket(tcp_socket);
        tcp_socket = INVALID_SOCKET;
    }
    freeaddrinfo(res);
    if (tcp_socket == INVALID_SOCKET) {
        fprintf(stderr, "HCI TCP: cannot connect to %s:%s\n", tcp_host, tcp_port);
        return -1;
    }
    setsockopt(tcp_socket, IPPROTO_TCP, TCP_NODELAY, (const char *)&nodelay, sizeof(nodelay));
    fprintf(stderr, "HCI TCP: connected to %s:%s\n", tcp_host, tcp_port);

    memset(&overlapped_read, 0, sizeof(overlapped_read));
    memset(&overlapped_write, 0, sizeof(overlapped_write));
    overlapped_read.hEvent = WSACreateEvent();
    overlapped_write.hEvent = WSACreateEvent();
    btstack_run_loop_set_data_source_handle(&data_source_read, overlapped_read.hEvent);
    btstack_run_loop_set_data_source_handle(&data_source_write, overlapped_write.hEvent);
    btstack_run_loop_set_data_source_handler(&data_source_read, &process_read);
    btstack_run_loop_set_data_source_handler(&data_source_write, &process_write);
    btstack_run_loop_add_data_source(&data_source_read);
    btstack_run_loop_add_data_source(&data_source_write);
    return 0;
}

static int uart_close(void) {
    btstack_run_loop_remove_data_source(&data_source_read);
    btstack_run_loop_remove_data_source(&data_source_write);
    if (tcp_socket != INVALID_SOCKET) {
        closesocket(tcp_socket);
        tcp_socket = INVALID_SOCKET;
    }
    if (overlapped_read.hEvent) WSACloseEvent(overlapped_read.hEvent);
    if (overlapped_write.hEvent) WSACloseEvent(overlapped_write.hEvent);
    overlapped_read.hEvent = NULL;
    overlapped_write.hEvent = NULL;
    WSACleanup();
    return 0;
}

static void uart_set_block_received(void (*handler)(void)) { block_received = handler; }
static void uart_set_block_sent(void (*handler)(void)) { block_sent = handler; }
static int uart_set_baudrate(uint32_t baudrate) { (void)baudrate; return 0; }
static int uart_set_parity(int parity) { (void)parity; return 0; }
static int uart_set_flowcontrol(int flowcontrol) { (void)flowcontrol; return 0; }

static void uart_receive_block(uint8_t *buffer, uint16_t len) {
    read_buffer = buffer;
    read_len = len;
    read_pos = 0;
    start_read();
}

static void uart_send_block(const uint8_t *buffer, uint16_t len) {
    write_buffer = buffer;
    write_len = len;
    write_pos = 0;
    start_write();
}

const btstack_uart_t *btstack_uart_tcp_windows_instance(void) {
    static const btstack_uart_t driver = {
        /* init */                      &uart_init,
        /* open */                      &uart_open,
        /* close */                     &uart_close,
        /* set_block_received */        &uart_set_block_received,
        /* set_block_sent */            &uart_set_block_sent,
        /* set_baudrate */              &uart_set_baudrate,
        /* set_parity */                &uart_set_parity,
        /* set_flowcontrol */           &uart_set_flowcontrol,
        /* receive_block */             &uart_receive_block,
        /* send_block */                &uart_send_block,
        /* get_supported_sleep_modes */ NULL,
        /* set_sleep */                 NULL,
        /* set_wakeup_handler */        NULL,
        /* set_frame_received */        NULL,
        /* set_frame_sent */            NULL,
        /* receive_frame */             NULL,
        /* send_frame */                NULL,
    };
    return &driver;
}
