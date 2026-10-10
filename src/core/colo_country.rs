//! 机场码（Cloudflare colo）→ 国家/地区码 映射，用于「地区配额续测」按国家清点节点。
//! 用户既可用 3 字母机场码（HKG）也可用 2 字母国家/地区码（US）作为配额键。

/// (机场码, 国家/地区码 ISO-3166-1 alpha-2)
const COLO_COUNTRY: &[(&str, &str)] = &[
    // 港澳台
    ("HKG", "HK"), ("TPE", "TW"), ("MFM", "MO"),
    // 日本
    ("NRT", "JP"), ("KIX", "JP"), ("ITM", "JP"), ("FUK", "JP"), ("HND", "JP"), ("CTS", "JP"), ("OKA", "JP"),
    // 韩国
    ("ICN", "KR"), ("GMP", "KR"), ("PUS", "KR"),
    // 东南亚
    ("SIN", "SG"), ("BKK", "TH"), ("HAN", "VN"), ("SGN", "VN"), ("MNL", "PH"), ("CGK", "ID"),
    ("KUL", "MY"), ("RGN", "MM"), ("PNH", "KH"), ("BWN", "BN"), ("DPS", "ID"),
    // 南亚
    ("BOM", "IN"), ("DEL", "IN"), ("MAA", "IN"), ("BLR", "IN"), ("HYD", "IN"), ("CCU", "IN"),
    ("CMB", "LK"), ("DAC", "BD"), ("KTM", "NP"), ("PKK", "PK"), ("KHI", "PK"),
    // 大洋洲
    ("SYD", "AU"), ("MEL", "AU"), ("BNE", "AU"), ("PER", "AU"), ("ADL", "AU"), ("AKL", "NZ"), ("WLG", "NZ"),
    // 美国
    ("LAX", "US"), ("SJC", "US"), ("SEA", "US"), ("SFO", "US"), ("PDX", "US"), ("SAN", "US"),
    ("PHX", "US"), ("LAS", "US"), ("EWR", "US"), ("IAD", "US"), ("BOS", "US"), ("PHL", "US"),
    ("ATL", "US"), ("MIA", "US"), ("MCO", "US"), ("ORD", "US"), ("DFW", "US"), ("IAH", "US"),
    ("DEN", "US"), ("MSP", "US"), ("DTW", "US"), ("STL", "US"), ("MCI", "US"), ("SLC", "US"),
    ("HNL", "US"), ("ANC", "US"),
    // 加拿大
    ("YYZ", "CA"), ("YVR", "CA"), ("YUL", "CA"), ("YYC", "CA"),
    // 欧洲
    ("LHR", "GB"), ("MAN", "GB"), ("CDG", "FR"), ("MRS", "FR"), ("FRA", "DE"), ("MUC", "DE"),
    ("DUS", "DE"), ("HAM", "DE"), ("BER", "DE"), ("AMS", "NL"), ("BRU", "BE"), ("ZRH", "CH"),
    ("VIE", "AT"), ("MAD", "ES"), ("BCN", "ES"), ("MXP", "IT"), ("FCO", "IT"), ("ATH", "GR"),
    ("LIS", "PT"), ("ARN", "SE"), ("CPH", "DK"), ("OSL", "NO"), ("HEL", "FI"), ("WAW", "PL"),
    ("PRG", "CZ"), ("BUD", "HU"), ("OTP", "RO"), ("SOF", "BG"), ("DUB", "IE"), ("MIL", "IT"),
    // 中东
    ("DXB", "AE"), ("AUH", "AE"), ("TLV", "IL"), ("BAH", "BH"), ("AMM", "JO"), ("KWI", "KW"),
    ("DOH", "QA"), ("MCT", "OM"), ("RUH", "SA"), ("JED", "SA"), ("IST", "TR"),
    // 南美
    ("GRU", "BR"), ("GIG", "BR"), ("EZE", "AR"), ("BOG", "CO"), ("LIM", "PE"), ("SCL", "CL"),
    ("MDE", "CO"), ("POA", "BR"),
    // 非洲
    ("JNB", "ZA"), ("CPT", "ZA"), ("CAI", "EG"), ("LOS", "NG"), ("NBO", "KE"), ("ACC", "GH"),
    ("CAS", "MA"), ("ADD", "ET"),
];

/// 机场码 → 国家/地区码；未知返回 None
pub fn country_of(colo: &str) -> Option<&'static str> {
    let c = colo.trim().to_uppercase();
    COLO_COUNTRY
        .iter()
        .find(|(code, _)| *code == c)
        .map(|(_, country)| *country)
}

/// 判断某节点的机场码是否落在配额键 `key` 指定的地区内。
/// `key` 可以是机场码（HKG）或国家/地区码（US），不区分大小写。
/// 节点机场码为空（未识别）时一律不匹配。
pub fn region_matches(key: &str, colo: &str) -> bool {
    let colo = colo.trim().to_uppercase();
    if colo.is_empty() {
        return false;
    }
    let key = key.trim().to_uppercase();
    if key.is_empty() {
        return false;
    }
    // 直接匹配机场码
    if key == colo {
        return true;
    }
    // 匹配国家/地区码
    match country_of(&colo) {
        Some(country) => country == key,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_by_colo() {
        assert!(region_matches("HKG", "HKG"));
        assert!(region_matches("hkg", "HKG"));
        assert!(!region_matches("HKG", "TPE"));
    }

    #[test]
    fn match_by_country() {
        assert!(region_matches("US", "LAX"));
        assert!(region_matches("us", "SJC"));
        assert!(region_matches("JP", "NRT"));
        assert!(!region_matches("US", "HKG"));
    }

    #[test]
    fn empty_colo_never_matches() {
        assert!(!region_matches("US", ""));
        assert!(!region_matches("HKG", ""));
    }

    #[test]
    fn unknown_colo_only_matches_itself() {
        assert!(region_matches("ZZZ", "ZZZ"));
        assert!(!region_matches("US", "ZZZ"));
    }
}
