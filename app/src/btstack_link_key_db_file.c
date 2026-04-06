/*
 * File-backed link key database for BTstack
 *
 * Stores all link keys in a single text file (link_keys.txt).
 * Format: one entry per line — "XX-XX-XX-XX-XX-XX <32-hex-key> <type>"
 *
 * SPDX-License-Identifier: MIT
 */

#include "btstack_link_key_db_file.h"
#include "btstack_util.h"
#include "btstack_debug.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
#endif

#define MAX_LINK_KEYS 32
#define ADDR_STR_LEN  18  /* "XX-XX-XX-XX-XX-XX\0" */
#define KEY_HEX_LEN   32  /* 16 bytes * 2 hex chars */
#define LINE_BUF_LEN  64  /* addr(17) + space(1) + key(32) + space(1) + type(1) + NL(1) + pad */

/* Storage directory set by caller */
static char db_dir[260];  /* MAX_PATH on Windows */

/* Full path to the link_keys.txt file */
static char db_filepath[260];

/* In-memory cache of link keys (loaded from file) */
typedef struct {
    bd_addr_t       addr;
    link_key_t      key;
    link_key_type_t type;
    int             used;
} link_key_entry_t;

static link_key_entry_t entries[MAX_LINK_KEYS];
static int num_entries = 0;

static void build_filepath(void) {
    snprintf(db_filepath, sizeof(db_filepath), "%s\\link_keys.txt", db_dir);
}

static char *addr_to_dash_str(const bd_addr_t addr) {
    return bd_addr_to_str_with_delimiter(addr, '-');
}

static int parse_hex_byte(const char *s) {
    unsigned int val = 0;
    if (sscanf(s, "%2x", &val) != 1) return -1;
    return (int)val;
}

/* Load all entries from the file into the in-memory cache */
static void load_from_file(void) {
    FILE *f = NULL;
    num_entries = 0;

    build_filepath();
#ifdef _MSC_VER
    fopen_s(&f, db_filepath, "r");
#else
    f = fopen(db_filepath, "r");
#endif
    if (!f) return;

    char line[128];
    while (fgets(line, sizeof(line), f) && num_entries < MAX_LINK_KEYS) {
        /* Skip comment lines */
        if (line[0] == '#' || line[0] == '\n' || line[0] == '\r') continue;

        /* Parse: "XX-XX-XX-XX-XX-XX XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX T" */
        bd_addr_t addr;
        if (sscanf_bd_addr(&line[0], addr) == 0) continue;

        /* Link key starts at offset 18 (17 chars for addr + 1 space) */
        if (strlen(line) < 18 + KEY_HEX_LEN + 2) continue;
        const char *key_str = &line[18];

        link_key_t key;
        int i;
        int valid = 1;
        for (i = 0; i < 16; i++) {
            int b = parse_hex_byte(&key_str[i * 2]);
            if (b < 0) { valid = 0; break; }
            key[i] = (uint8_t)b;
        }
        if (!valid) continue;

        /* Type digit after key + space */
        int type_val = 0;
        if (sscanf(&key_str[KEY_HEX_LEN + 1], "%d", &type_val) != 1) continue;

        memcpy(entries[num_entries].addr, addr, 6);
        memcpy(entries[num_entries].key, key, 16);
        entries[num_entries].type = (link_key_type_t)type_val;
        entries[num_entries].used = 1;
        num_entries++;
    }
    fclose(f);
}

/* Write all in-memory entries to the file (atomic via temp + rename) */
static void save_to_file(void) {
    build_filepath();

    char tmp_path[268];
    snprintf(tmp_path, sizeof(tmp_path), "%s.tmp", db_filepath);

    FILE *f = NULL;
#ifdef _MSC_VER
    fopen_s(&f, tmp_path, "w");
#else
    f = fopen(tmp_path, "w");
#endif
    if (!f) {
        log_error("btstack_link_key_db_file: failed to write %s", tmp_path);
        return;
    }

    fprintf(f, "# BTstack link key database — do not edit while app is running\n");

    int i;
    for (i = 0; i < num_entries; i++) {
        if (!entries[i].used) continue;

        char *addr_str = addr_to_dash_str(entries[i].addr);

        /* Write key as 32 hex chars */
        char key_hex[KEY_HEX_LEN + 1];
        int j;
        for (j = 0; j < 16; j++) {
            sprintf(&key_hex[j * 2], "%02x", entries[i].key[j]);
        }
        key_hex[KEY_HEX_LEN] = '\0';

        fprintf(f, "%s %s %d\n", addr_str, key_hex, (int)entries[i].type);
    }

    fclose(f);

    /* Atomic replace */
#ifdef _WIN32
    MoveFileExA(tmp_path, db_filepath, MOVEFILE_REPLACE_EXISTING);
#else
    rename(tmp_path, db_filepath);
#endif
}

/* Find entry index for addr, or -1 */
static int find_entry(const bd_addr_t addr) {
    int i;
    for (i = 0; i < num_entries; i++) {
        if (entries[i].used && memcmp(entries[i].addr, addr, 6) == 0)
            return i;
    }
    return -1;
}

/* ---- btstack_link_key_db_t interface implementation ---- */

static void db_open(void) {
    load_from_file();
    log_info("btstack_link_key_db_file: loaded %d link key(s) from %s", num_entries, db_filepath);
}

static void db_set_local_bd_addr(bd_addr_t bd_addr) {
    (void)bd_addr;  /* single-adapter app, no per-adapter segregation */
}

static void db_close(void) {
    /* nothing to do — already saved on each put/delete */
}

static int db_get_link_key(bd_addr_t bd_addr, link_key_t link_key, link_key_type_t *type) {
    int idx = find_entry(bd_addr);
    if (idx < 0) return 0;

    memcpy(link_key, entries[idx].key, 16);
    *type = entries[idx].type;
    log_info("btstack_link_key_db_file: found link key for %s", addr_to_dash_str(bd_addr));
    return 1;
}

static void db_put_link_key(bd_addr_t bd_addr, link_key_t link_key, link_key_type_t type) {
    int idx = find_entry(bd_addr);
    if (idx < 0) {
        /* Add new entry */
        if (num_entries >= MAX_LINK_KEYS) {
            /* Evict oldest (first) entry */
            memmove(&entries[0], &entries[1], sizeof(link_key_entry_t) * (MAX_LINK_KEYS - 1));
            num_entries = MAX_LINK_KEYS - 1;
        }
        idx = num_entries++;
    }

    memcpy(entries[idx].addr, bd_addr, 6);
    memcpy(entries[idx].key, link_key, 16);
    entries[idx].type = type;
    entries[idx].used = 1;

    save_to_file();
    log_info("btstack_link_key_db_file: stored link key for %s", addr_to_dash_str(bd_addr));
}

static void db_delete_link_key(bd_addr_t bd_addr) {
    int idx = find_entry(bd_addr);
    if (idx < 0) return;

    /* Shift remaining entries down */
    if (idx < num_entries - 1) {
        memmove(&entries[idx], &entries[idx + 1],
                sizeof(link_key_entry_t) * (num_entries - 1 - idx));
    }
    num_entries--;

    save_to_file();
    log_info("btstack_link_key_db_file: deleted link key for %s", addr_to_dash_str(bd_addr));
}

/* ---- Iterator ---- */

typedef struct {
    int index;
} file_iterator_t;

static int db_iterator_init(btstack_link_key_iterator_t *it) {
    file_iterator_t *ctx = (file_iterator_t *)malloc(sizeof(file_iterator_t));
    if (!ctx) return 0;
    ctx->index = 0;
    it->context = ctx;
    return 1;
}

static int db_iterator_get_next(btstack_link_key_iterator_t *it, bd_addr_t bd_addr,
                                link_key_t link_key, link_key_type_t *type) {
    file_iterator_t *ctx = (file_iterator_t *)it->context;
    while (ctx->index < num_entries) {
        int i = ctx->index++;
        if (!entries[i].used) continue;
        memcpy(bd_addr, entries[i].addr, 6);
        memcpy(link_key, entries[i].key, 16);
        *type = entries[i].type;
        return 1;
    }
    return 0;
}

static void db_iterator_done(btstack_link_key_iterator_t *it) {
    free(it->context);
    it->context = NULL;
}

/* ---- Singleton ---- */

static const btstack_link_key_db_t btstack_link_key_db_file = {
    &db_open,
    &db_set_local_bd_addr,
    &db_close,
    &db_get_link_key,
    &db_put_link_key,
    &db_delete_link_key,
    &db_iterator_init,
    &db_iterator_get_next,
    &db_iterator_done,
};

void btstack_link_key_db_file_set_path(const char *dir_path) {
    strncpy(db_dir, dir_path, sizeof(db_dir) - 1);
    db_dir[sizeof(db_dir) - 1] = '\0';
}

const btstack_link_key_db_t * btstack_link_key_db_file_instance(void) {
    return &btstack_link_key_db_file;
}
