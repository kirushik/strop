use std::collections::BTreeMap;
use std::io::Read;

pub const LATEST_URL: &str =
    "https://github.com/kirushik/strop/releases/latest/download/latest.json";
const HOSTS: &[&str] = &["github.com", "objects.githubusercontent.com",
    "release-assets.githubusercontent.com"];

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

pub trait Fetcher: Send + Sync {
    fn get(&self, url: &str, limit: u64) -> Result<Response, String>;
}

pub struct NetworkFetcher;

impl Fetcher for NetworkFetcher {
    fn get(&self, url: &str, limit: u64) -> Result<Response, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder().max_redirects(0).build().into();
        let response = agent.get(url).call().map_err(|e| format!("update fetch failed: {e}"))?;
        let status = response.status().as_u16();
        let headers = response.headers().iter().filter_map(|(name, value)| {
            value.to_str().ok().map(|v| (name.as_str().to_ascii_lowercase(), v.to_owned()))
        }).collect();
        let mut body = Vec::new();
        // A redirect's body is never wanted — only its Location header. Not
        // reading it means a hop can't spend the transfer budget at all.
        if !(300..400).contains(&status) {
            response.into_body().as_reader().take(limit + 1).read_to_end(&mut body)
                .map_err(|e| format!("update response read failed: {e}"))?;
            if body.len() as u64 > limit { return Err("update response exceeds size limit".into()); }
        }
        Ok(Response { status, headers, body })
    }
}

pub fn fetch_following(fetcher: &dyn Fetcher, initial: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut url = initial.to_owned();
    // ONE cumulative byte budget for the whole redirect chain, enforced
    // here regardless of what the fetcher returns — per-hop budgets would
    // multiply the cap by the hop count. The caps must bound the *check*,
    // not the request.
    let mut remaining = limit;
    for _ in 0..=8 {
        validate_url(&url)?;
        let response = fetcher.get(&url, remaining)?;
        if response.body.len() as u64 > remaining {
            return Err("update response exceeds size limit".into());
        }
        remaining -= response.body.len() as u64;
        if (300..400).contains(&response.status) {
            let location = response.headers.get("location")
                .ok_or_else(|| "redirect has no Location header".to_owned())?;
            if !location.starts_with("https://") {
                return Err("redirect Location must be an absolute HTTPS URL".into());
            }
            url = location.clone();
            continue;
        }
        if response.status != 200 { return Err(format!("update server returned {}", response.status)); }
        return Ok(response.body);
    }
    Err("too many update redirects".into())
}

pub fn validate_url(url: &str) -> Result<(), String> {
    let rest = url.strip_prefix("https://").ok_or_else(|| "update URL is not HTTPS".to_owned())?;
    let authority = rest.split('/').next().unwrap_or("");
    if authority.contains('@') || authority.contains(':') || !HOSTS.contains(&authority) {
        return Err("update URL host is not allowed".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Scripted { responses: Mutex<Vec<(String, Response)>> }
    impl Fetcher for Scripted {
        fn get(&self, url: &str, _limit: u64) -> Result<Response, String> {
            let mut responses = self.responses.lock().unwrap();
            let (expected, response) = responses.remove(0);
            assert_eq!(url, expected);
            Ok(response)
        }
    }
    fn response(status: u16, location: Option<&str>, body: &[u8]) -> Response {
        let mut headers = BTreeMap::new();
        if let Some(location) = location { headers.insert("location".into(), location.into()); }
        Response { status, headers, body: body.to_vec() }
    }
    #[test]
    fn url_allowlist_is_exact() {
        for host in HOSTS { assert!(validate_url(&format!("https://{host}/x")).is_ok()); }
        assert!(validate_url("http://github.com/x").is_err());
        assert!(validate_url("https://github.com.evil/x").is_err());
        assert!(validate_url("https://github.com@evil/x").is_err());
    }

    #[test]
    fn transfer_budget_is_cumulative_across_the_chain() {
        // A hop that returns bytes spends the ONE budget; it never resets.
        let fetcher = Scripted { responses: Mutex::new(vec![
            ("https://github.com/start".into(), response(302,
                Some("https://objects.githubusercontent.com/mid"), b"xxxxxxxx")),
            ("https://objects.githubusercontent.com/mid".into(), response(200, None, b"payload")),
        ]) };
        // Budget 10: the rogue 8-byte redirect body leaves 2, so the
        // 7-byte payload must be refused.
        assert!(fetch_following(&fetcher, "https://github.com/start", 10).is_err());
        // Empty redirect bodies (the honest case) spend nothing.
        let fetcher = Scripted { responses: Mutex::new(vec![
            ("https://github.com/start".into(), response(302,
                Some("https://objects.githubusercontent.com/mid"), b"")),
            ("https://objects.githubusercontent.com/mid".into(), response(200, None, b"payload")),
        ]) };
        assert_eq!(fetch_following(&fetcher, "https://github.com/start", 7).unwrap(), b"payload");
        // And a body over the whole budget is refused even with no redirects.
        let fetcher = Scripted { responses: Mutex::new(vec![
            ("https://github.com/start".into(), response(200, None, b"12345678901")),
        ]) };
        assert!(fetch_following(&fetcher, "https://github.com/start", 10).is_err());
    }

    #[test]
    fn every_redirect_hop_is_validated_without_a_socket() {
        let fetcher = Scripted { responses: Mutex::new(vec![
            ("https://github.com/start".into(), response(302,
                Some("https://objects.githubusercontent.com/middle"), b"")),
            ("https://objects.githubusercontent.com/middle".into(), response(307,
                Some("https://release-assets.githubusercontent.com/end"), b"")),
            ("https://release-assets.githubusercontent.com/end".into(), response(200, None, b"ok")),
        ]) };
        assert_eq!(fetch_following(&fetcher, "https://github.com/start", 2).unwrap(), b"ok");
        for bad in ["http://github.com/end", "https://github.com.evil/end",
            "https://github.com@evil.example/end"] {
            let fetcher = Scripted { responses: Mutex::new(vec![
                ("https://github.com/start".into(), response(302, Some(bad), b"")),
            ]) };
            assert!(fetch_following(&fetcher, "https://github.com/start", 2).is_err());
        }
    }
}
