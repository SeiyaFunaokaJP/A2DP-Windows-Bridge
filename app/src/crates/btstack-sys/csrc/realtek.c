/*
 * realtek.c - BTstack's Realtek chipset driver, plus a read of its USB
 * firmware table
 *
 * fw_patch_table_usb is static in btstack_chipset_realtek.c and BTstack only
 * exports its VID:PID pairs. The app also needs each PID's firmware name to
 * tell the chip family (0x4853 and 0xB85B are both RTL8852BU), so this file
 * compiles the driver itself and reads the table from the same unit.
 *
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

#include "btstack_chipset_realtek.c"

/* The firmware name (no extension, e.g. "rtl8852bu_fw") of entry `index` of
 * btstack_chipset_realtek_get_vendor_product_id's table */
const char *a2dpwb_realtek_usb_fw_name(uint16_t index) {
    if (index >= btstack_chipset_realtek_get_num_usb_controllers()) return NULL;
    return fw_patch_table_usb[index].patch_name;
}

/* BTstack's driver opens "${folder}/${name}" with no extension, while the
 * files carry .bin (linux-firmware, the app's download). Its init builds the
 * paths again at each power on, so this init runs it and then appends .bin
 * where that file is there; an extension-less file alone is still taken. */
static void a2dpwb_append_bin(char *path, size_t size) {
    size_t len = strlen(path);
    if (len + 5 > size) return;
    memcpy(&path[len], ".bin", 5);
    FILE *f = fopen(path, "rb");
    if (f != NULL) {
        fclose(f);
        return;
    }
    path[len] = '\0';
}

static void a2dpwb_realtek_init(const void *config) {
    chipset_init(config);
    /* The USB lookup by product ID: it only names the files here, loading
     * them later (the UART lookup has loaded them already) */
    if (product_id != 0 && patch_usb != NULL) {
        a2dpwb_append_bin(firmware_file, sizeof(firmware_file));
        a2dpwb_append_bin(config_file, sizeof(config_file));
    }
}

static btstack_chipset_t a2dpwb_realtek;

/* BTstack's Realtek chipset with the init above */
const btstack_chipset_t *a2dpwb_realtek_instance(void) {
    a2dpwb_realtek = btstack_chipset_realtek;
    a2dpwb_realtek.init = a2dpwb_realtek_init;
    return &a2dpwb_realtek;
}
