use std::cmp::Ordering;

/// Только релизные ID вида `1.21` / `1.21.5` (без pre/rc/snapshot/недельных).
pub fn is_release_style_mc_version(v: &str) -> bool {
    let mut parts = v.split('.');
    let a = parts.next();
    let b = parts.next();
    let c = parts.next();
    if parts.next().is_some() {
        return false;
    }
    let seg_ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    match (a, b, c) {
        (Some(x), Some(y), None) => seg_ok(x) && seg_ok(y),
        (Some(x), Some(y), Some(z)) => seg_ok(x) && seg_ok(y) && seg_ok(z),
        _ => false,
    }
}

fn cmp_mc_version_seg(a: &str, b: &str) -> Ordering {
    let na: String = a.chars().take_while(|c| c.is_ascii_digit()).collect();
    let nb: String = b.chars().take_while(|c| c.is_ascii_digit()).collect();
    let ra: &str = a.trim_start_matches(|c: char| c.is_ascii_digit());
    let rb: &str = b.trim_start_matches(|c: char| c.is_ascii_digit());
    if !na.is_empty() && !nb.is_empty() {
        let pa = na.parse::<u32>().unwrap_or(0);
        let pb = nb.parse::<u32>().unwrap_or(0);
        match pa.cmp(&pb) {
            Ordering::Equal => ra.cmp(rb),
            o => o,
        }
    } else {
        a.cmp(b)
    }
}

pub fn cmp_mc_version_asc(a: &str, b: &str) -> Ordering {
    let mut ia = a.split('.');
    let mut ib = b.split('.');
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(sa), Some(sb)) => {
                let o = cmp_mc_version_seg(sa, sb);
                if o != Ordering::Equal {
                    return o;
                }
            }
        }
    }
}

pub fn sort_mc_versions_desc(versions: &mut [String]) {
    versions.sort_by(|a, b| cmp_mc_version_asc(a, b).reverse());
}

pub fn mc_version_precedes(a: &str, b: &str) -> bool {
    cmp_mc_version_asc(a, b) == Ordering::Less
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_style_recognizes_basic_versions() {
        assert!(is_release_style_mc_version("1.21"));
        assert!(is_release_style_mc_version("1.21.5"));
        assert!(!is_release_style_mc_version("1.21.5-pre1"));
        assert!(!is_release_style_mc_version("24w14a"));
        assert!(!is_release_style_mc_version("1"));
        assert!(!is_release_style_mc_version(""));
    }

    #[test]
    fn cmp_mc_version_orders_correctly() {
        assert_eq!(cmp_mc_version_asc("1.20", "1.21"), Ordering::Less);
        assert_eq!(cmp_mc_version_asc("1.21", "1.20"), Ordering::Greater);
        assert_eq!(cmp_mc_version_asc("1.21.5", "1.21.5"), Ordering::Equal);
        assert_eq!(cmp_mc_version_asc("1.21", "1.21.5"), Ordering::Less);
        assert_eq!(cmp_mc_version_asc("1.21.10", "1.21.9"), Ordering::Greater);
    }

    #[test]
    fn sort_versions_descending() {
        let mut versions = vec![
            "1.20.1".to_string(),
            "1.21.5".to_string(),
            "1.19.2".to_string(),
            "1.21".to_string(),
        ];
        sort_mc_versions_desc(&mut versions);
        assert_eq!(
            versions,
            vec![
                "1.21.5".to_string(),
                "1.21".to_string(),
                "1.20.1".to_string(),
                "1.19.2".to_string(),
            ]
        );
    }

    #[test]
    fn mc_version_precedes_works() {
        assert!(mc_version_precedes("1.20", "1.21"));
        assert!(!mc_version_precedes("1.21", "1.20"));
        assert!(!mc_version_precedes("1.21", "1.21"));
    }
}
