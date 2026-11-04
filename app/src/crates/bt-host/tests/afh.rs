//! Tests of the AFH classification (src/afh.rs)

use bt_host::afh;

use afh::*;

/// Bluetooth channels set in a map
fn usable_channels(map: &[u8; 10]) -> Vec<usize> {
    (0..CHANNELS).filter(|&n| map[n / 8] & (1 << (n % 8)) != 0).collect()
}

#[test]
fn off_and_empty_pass_nothing() {
    for policy in ["off", ""] {
        let c = classify(policy, || panic!("no survey for {policy:?}"));
        assert!(!c.passed);
        assert_eq!(c.usable, 79);
        assert_eq!(c.map, [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
        assert!(c.avoided_wifi.is_empty());
    }
}

#[test]
fn wifi_channel_marks_its_width() {
    // Channel 6: center 2437 MHz, +-11 MHz = 2426..2448 = BT channels 24..46
    let c = avoid_wifi(&[6]);
    assert!(c.passed);
    assert_eq!(c.avoided_wifi, [6]);
    assert_eq!(c.usable, 79 - 23);
    let usable = usable_channels(&c.map);
    assert!(usable.contains(&23) && usable.contains(&47));
    assert!((24..=46).all(|n| !usable.contains(&n)));
    // Channel 1: 2401..2423 MHz, BT channels 0..21
    let c = avoid_wifi(&[1]);
    assert_eq!(c.usable, 79 - 22);
    assert_eq!(usable_channels(&c.map)[0], 22);
    // Channel 14: center 2484 MHz, 2473..2480 = BT channels 71..78
    let c = avoid_wifi(&[14]);
    assert_eq!(c.usable, 79 - 8);
    assert_eq!(*usable_channels(&c.map).last().unwrap(), 70);
}

#[test]
fn keeps_at_least_twenty_channels() {
    // 1 and 6 leave 34; 11 would leave 11, so it is skipped
    let c = classify("1,6,11", Vec::new);
    assert_eq!(c.avoided_wifi, [1, 6]);
    assert_eq!(c.usable, 79 - 22 - 23);
    assert!(c.usable >= MIN_CHANNELS);
    assert_eq!(count(&c.map), c.usable);
}

#[test]
fn manual_list_parsing_is_forgiving() {
    assert_eq!(Policy::parse("6,11"), Policy::Manual(vec![6, 11]));
    assert_eq!(Policy::parse("6, 11"), Policy::Manual(vec![6, 11]));
    assert_eq!(Policy::parse(" 6;11 x"), Policy::Manual(vec![6, 11]));
    assert_eq!(Policy::parse("-3,+4"), Policy::Manual(vec![-3, 4]));
    assert_eq!(Policy::parse("abc"), Policy::Manual(vec![]));
    assert_eq!(Policy::parse("auto"), Policy::Auto);
    assert_eq!(Policy::parse("off"), Policy::Off);
    // Out of range and repeated channels are skipped
    let c = classify("0,15,-3,6,6,99999999999999999999", Vec::new);
    assert_eq!(c.avoided_wifi, [6]);
    // A list with no valid channel passes nothing
    assert!(!classify("abc", Vec::new).passed);
}

#[test]
fn auto_avoids_only_the_connected_channel() {
    // Connected on 2.4 GHz channel 11: BT channels 49..71 marked bad
    let c = classify("auto", || vec![11]);
    assert!(c.passed);
    assert_eq!(c.avoided_wifi, [11]);
    assert_eq!(c.usable, 79 - 23);
    let usable = usable_channels(&c.map);
    assert!(usable.contains(&48) && usable.contains(&72));
    assert!((49..=71).all(|n| !usable.contains(&n)));
    assert_eq!(auto_channels(&[11]), [11]);
}

#[test]
fn auto_passes_nothing_without_2_4_ghz_wifi() {
    // Disconnected, on 5 / 6 GHz or no Wi-Fi: the survey returns nothing,
    // however loud the access points around are
    let c = classify("auto", Vec::new);
    assert!(!c.passed);
    assert_eq!(c, Classification::all_usable());
    assert!(auto_channels(&[]).is_empty());
}

#[test]
fn auto_with_two_interfaces_keeps_twenty_channels() {
    // Two adapters on 1 and 6, and a repeat: each channel once
    let c = classify("auto", || vec![1, 6, 1]);
    assert_eq!(c.avoided_wifi, [1, 6]);
    assert_eq!(c.usable, 79 - 22 - 23);
    // A third on 11 would leave fewer than 20: skipped
    let c = classify("auto", || vec![1, 6, 11]);
    assert_eq!(c.avoided_wifi, [1, 6]);
    assert!(c.usable >= MIN_CHANNELS);
}

#[test]
fn describe_lists_channels() {
    assert_eq!(describe(&[6, 11]), "6, 11");
    assert_eq!(describe(&[1]), "1");
    assert_eq!(describe(&[]), "");
}

#[test]
fn count_ignores_the_reserved_bit() {
    assert_eq!(count(&[0xFF; 10]), 79);
    assert_eq!(count(&[0; 10]), 0);
    assert_eq!(wifi_center_mhz(1), 2412);
    assert_eq!(wifi_center_mhz(13), 2472);
    assert_eq!(wifi_center_mhz(14), 2484);
}

#[test]
fn unused_ranges_name_the_gaps() {
    assert_eq!(unused_ranges(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]), "");
    // Wi-Fi channel 6 avoided: Bluetooth channels 24-46
    assert_eq!(unused_ranges(&avoid_wifi(&[6]).map), "24-46");
    // A single channel, and the last ones (the reserved bit 79 is no channel)
    let mut map = [0xFF; 10];
    map[0] &= !1;
    map[9] = 0x3F;
    assert_eq!(unused_ranges(&map), "0, 78");
    assert_eq!(unused_ranges(&[0; 10]), "0-78");
}
