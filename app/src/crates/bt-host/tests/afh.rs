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

// ---- blend (experimental) ----

/// A controller map that hops on every channel except `out`
fn map_without(out: impl IntoIterator<Item = usize>) -> [u8; 10] {
    let mut map = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F];
    for n in out {
        map[n / 8] &= !(1 << (n % 8));
    }
    map
}

/// A history of `readings` identical controller maps
fn history(map: [u8; 10], readings: usize) -> ControllerHistory {
    let mut h = ControllerHistory::default();
    for _ in 0..readings {
        h.record(&map, None);
    }
    h
}

fn input<'a>(
    strictness: Strictness,
    aps: Option<&'a [ApReading]>,
    controller: &'a ControllerHistory,
) -> BlendInput<'a> {
    BlendInput { strictness, connected: &[], aps, controller, previous: None }
}

/// Wi-Fi channel 6 covers Bluetooth channels 24-46
const CH6: std::ops::RangeInclusive<usize> = 24..=46;

#[test]
fn blend_policies_parse_and_need_developer_mode() {
    assert_eq!(Policy::parse("blend-mild"), Policy::Blend(Strictness::Mild));
    assert_eq!(Policy::parse("blend-medium"), Policy::Blend(Strictness::Medium));
    assert_eq!(Policy::parse("blend-strong"), Policy::Blend(Strictness::Strong));
    assert_eq!(effective_policy("blend-medium", true), "blend-medium");
    assert_eq!(effective_policy("blend-medium", false), "auto");
    for p in ["auto", "off", "6,11"] {
        assert_eq!(effective_policy(p, false), p);
    }
    // classify alone has no scan or history: blend runs as auto there
    assert_eq!(classify("blend-strong", || vec![11]), classify("auto", || vec![11]));
}

#[test]
fn wifi_score_follows_the_summed_level_within_11_mhz() {
    let loud = [ApReading { channel: 6, rssi: -50 }];
    assert!(CH6.clone().all(|n| approx(wifi_score(n, &loud), 1.0)));
    assert_eq!(wifi_score(23, &loud), 0.0);
    assert_eq!(wifi_score(47, &loud), 0.0);
    assert!(approx(wifi_score(30, &[ApReading { channel: 6, rssi: -70 }]), 0.5));
    assert_eq!(wifi_score(30, &[ApReading { channel: 6, rssi: -95 }]), 0.0);
    // Two access points at -73 dBm add up to about -70 dBm
    let two = [ApReading { channel: 6, rssi: -73 }, ApReading { channel: 7, rssi: -73 }];
    assert!((wifi_level_dbm(35, &two).unwrap() - -69.99).abs() < 0.1);
    assert!(wifi_level_dbm(0, &two).is_none());
    // Channel numbers out of range count for nothing
    assert!(wifi_level_dbm(30, &[ApReading { channel: 0, rssi: -40 }]).is_none());
}

#[test]
fn controller_history_counts_what_the_controller_left_out() {
    let h = history(map_without(CH6), MIN_OBSERVATIONS - 1);
    assert_eq!(h.left_out(30), None, "too few readings");
    let mut h = history(map_without(CH6), 3);
    h.record(&map_without([]), None);
    assert_eq!(h.left_out(30), Some(0.75));
    assert_eq!(h.left_out(10), Some(0.0));
    // Only the newest HISTORY_OBSERVATIONS count
    let mut h = history(map_without(CH6), HISTORY_OBSERVATIONS);
    for _ in 0..HISTORY_OBSERVATIONS {
        h.record(&map_without([]), None);
    }
    assert_eq!(h.left_out(30), Some(0.0));
    h.clear();
    assert_eq!(h.left_out(30), None);
    assert_eq!(h.readings(), 0);
}

#[test]
fn channels_the_host_marked_bad_are_not_observed() {
    // The host classification takes Wi-Fi 6 out, so the controller leaves
    // it out too: that says nothing about the channel
    let host = avoid_wifi(&[6]).map;
    let mut h = ControllerHistory::default();
    for _ in 0..10 {
        h.record(&host, Some(&host));
    }
    assert_eq!(h.left_out(30), None);
    assert_eq!(h.left_out(10), Some(0.0));
}

#[test]
fn readings_right_after_a_new_classification_are_skipped() {
    let mut h = ControllerHistory::default();
    h.host_changed();
    for _ in 0..SETTLE_READINGS {
        h.record(&map_without(CH6), None); // the old map, still in force
    }
    for _ in 0..MIN_OBSERVATIONS {
        h.record(&map_without([]), None);
    }
    assert_eq!(h.left_out(30), Some(0.0));
}

#[test]
fn old_observations_expire() {
    // A channel the host kept out for a long time loses its old readings
    let mut h = history(map_without(CH6), MIN_OBSERVATIONS);
    assert_eq!(h.left_out(30), Some(1.0));
    let host = avoid_wifi(&[6]).map;
    for _ in 0..HISTORY_MAX_AGE {
        h.record(&host, Some(&host));
    }
    assert_eq!(h.left_out(30), None);
    assert_eq!(h.left_out(10), Some(0.0));
}

#[test]
fn a_loud_access_point_the_controller_uses_well_is_kept() {
    // The 2026-10-17 case: a strong access point on 11 that the controller
    // hops over without trouble
    let aps = [ApReading { channel: 11, rssi: -55 }];
    let used = history(map_without([]), 10);
    for s in [Strictness::Mild, Strictness::Medium] {
        let c = blend(&input(s, Some(&aps), &used));
        assert!(!c.passed, "{s:?}");
        assert_eq!(c.usable, 79);
    }
    // Strong takes it: (0.875 + 0) / 2 >= 0.35
    let c = blend(&input(Strictness::Strong, Some(&aps), &used));
    assert_eq!(c.avoided_wifi, [11]);
    assert_eq!(c.usable, 79 - 23);
    // At -75 dBm (what v1 avoided) not even Strong does
    let quiet = [ApReading { channel: 11, rssi: -75 }];
    assert!(!blend(&input(Strictness::Strong, Some(&quiet), &used)).passed);
}

#[test]
fn both_signals_agreeing_mark_the_channel_bad() {
    let aps = [ApReading { channel: 6, rssi: -55 }];
    let h = history(map_without(CH6), 10);
    let c = blend(&input(Strictness::Mild, Some(&aps), &h));
    assert!(c.passed);
    assert_eq!(c.avoided_wifi, [6]);
    assert_eq!(c.usable, 79 - 23);
    assert_eq!(unused_ranges(&c.map), "24-46");
    let scores = channel_scores(&input(Strictness::Mild, Some(&aps), &h));
    assert_eq!(scores[30].controller, Some(1.0));
    assert!(approx(scores[30].wifi.unwrap(), 0.875));
    assert_eq!(scores[10].score, 0.0);
}

#[test]
fn one_signal_alone_counts_in_full() {
    // No scan (no location permission): the controller decides
    let h = history(map_without(CH6), 10);
    let c = blend(&input(Strictness::Mild, None, &h));
    assert_eq!(c.usable, 79 - 23);
    assert!(c.avoided_wifi.is_empty(), "no access point to name");
    // No history yet: the scan decides, -70 dBm scores 0.5
    let aps = [ApReading { channel: 6, rssi: -70 }];
    let none = ControllerHistory::default();
    assert!(!blend(&input(Strictness::Medium, Some(&aps), &none)).passed);
    assert_eq!(blend(&input(Strictness::Strong, Some(&aps), &none)).usable, 79 - 23);
    // Neither: nothing to pass
    assert_eq!(blend(&input(Strictness::Strong, None, &none)), Classification::all_usable());
}

#[test]
fn strictness_keeps_its_floor() {
    // The controller leaves out everything but a few channels: each
    // strictness stops at its floor, far above the specification's 20
    let h = history(map_without(0..75), 10);
    for (s, floor) in [(Strictness::Mild, 56), (Strictness::Medium, 48), (Strictness::Strong, 40)] {
        let c = blend(&input(s, None, &h));
        assert_eq!(c.usable, floor, "{s:?}");
        assert_eq!(s.floor(), floor);
        assert!(c.usable >= MIN_CHANNELS);
        // The channels the controller keeps stay usable
        let usable = usable_channels(&c.map);
        assert!((75..79).all(|n| usable.contains(&n)));
    }
}

#[test]
fn highest_scores_go_first() {
    // Wi-Fi 1 left out half the time, Wi-Fi 11 always: with room for only
    // part, 11 goes first
    let mut h = ControllerHistory::default();
    for i in 0..10 {
        let out: Vec<usize> = if i % 2 == 0 { (0..=21).chain(49..=71).collect() } else { (49..=71).collect() };
        h.record(&map_without(out), None);
    }
    let c = blend(&input(Strictness::Strong, None, &h));
    let usable = usable_channels(&c.map);
    assert!((49..=71).all(|n| !usable.contains(&n)));
    assert_eq!(c.usable, 40);
    assert_eq!((0..=21).filter(|n| !usable.contains(n)).count(), 79 - 23 - 40);
}

#[test]
fn the_pc_s_own_channel_is_avoided_as_auto_does() {
    let none = ControllerHistory::default();
    for s in [Strictness::Mild, Strictness::Medium, Strictness::Strong] {
        let c = blend(&BlendInput { strictness: s, connected: &[11], aps: None, controller: &none, previous: None });
        assert_eq!(c, classify("auto", || vec![11]), "{s:?}");
    }
    // Mild's floor of 56 leaves no room for more
    let h = history(map_without(CH6), 10);
    let c = blend(&BlendInput {
        strictness: Strictness::Mild,
        connected: &[11],
        aps: None,
        controller: &h,
        previous: None,
    });
    assert_eq!(c.usable, 79 - 23);
    assert_eq!(c.avoided_wifi, [11]);
    let channels = channel_scores(&BlendInput {
        strictness: Strictness::Mild,
        connected: &[11],
        aps: None,
        controller: &h,
        previous: None,
    });
    assert!(channels[60].own_wifi && !channels[30].own_wifi);
}

#[test]
fn a_channel_marked_bad_before_stays_bad_a_little_longer() {
    // Controller left Wi-Fi 6 out half the time: 0.5, below Medium's 0.55
    let mut h = ControllerHistory::default();
    for i in 0..10 {
        h.record(&if i % 2 == 0 { map_without(CH6) } else { map_without([]) }, None);
    }
    assert!(!blend(&input(Strictness::Medium, None, &h)).passed);
    // ... but within 0.1 of it, so a channel blend marked bad last time stays
    let previous = avoid_wifi(&[6]).map;
    let c = blend(&BlendInput { previous: Some(&previous), ..input(Strictness::Medium, None, &h) });
    assert_eq!(c.map, previous);
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}
