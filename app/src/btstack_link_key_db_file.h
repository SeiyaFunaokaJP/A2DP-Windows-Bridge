/*
 * File-backed link key database for BTstack
 *
 * Stores Bluetooth link keys in a single text file so that
 * previously paired devices can reconnect without pairing mode.
 *
 * SPDX-License-Identifier: MIT
 */

#ifndef BTSTACK_LINK_KEY_DB_FILE_H
#define BTSTACK_LINK_KEY_DB_FILE_H

#include "classic/btstack_link_key_db.h"

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Set the directory where link_keys.txt will be stored.
 * Must be called before hci_set_link_key_db().
 * dir_path: directory path (e.g. "C:\Users\...\AppData\Roaming\A2DPWB")
 */
void btstack_link_key_db_file_set_path(const char *dir_path);

const btstack_link_key_db_t * btstack_link_key_db_file_instance(void);

#ifdef __cplusplus
}
#endif

#endif /* BTSTACK_LINK_KEY_DB_FILE_H */
