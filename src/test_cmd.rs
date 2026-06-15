use crate::config::{save_config, AcsConfig, Provider};
use crate::errors::{AcsError, InteractiveError, ProviderError};
use colored::Colorize;
use dialoguer::Select;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

struct Probe {
    url: String,
    ms: Option<u128>,
}

fn probe_tcp(url: &str) -> Option<Duration> {
    let no_scheme = url.trim_start_matches("https://").trim_start_matches("http://");
    let host_port = no_scheme.split('/').next()?;
    let (host, port) = if let Some(colon) = host_port.rfind(':') {
        if let Ok(p) = host_port[colon + 1..].parse::<u16>() {
            (&host_port[..colon], p)
        } else {
            (host_port, if url.starts_with("https://") { 443 } else { 80 })
        }
    } else {
        (host_port, if url.starts_with("https://") { 443 } else { 80 })
    };
    let addr = format!("{}:{}", host, port).to_socket_addrs().ok()?.next()?;
    let start = Instant::now();
    TcpStream::connect_timeout(&addr, Duration::from_secs(5)).ok()?;
    Some(start.elapsed())
}

fn probe_all(urls: &[String]) -> Vec<Probe> {
    let handles: Vec<_> = urls
        .iter()
        .map(|u| {
            let u = u.clone();
            std::thread::spawn(move || Probe {
                ms: probe_tcp(&u).map(|d| d.as_micros()),
                url: u,
            })
        })
        .collect();
    let mut results: Vec<Probe> = handles
        .into_iter()
        .map(|h| h.join().unwrap_or_else(|_| Probe { url: String::new(), ms: None }))
        .collect();
    results.sort_by(|a, b| match (a.ms, b.ms) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    });
    results
}

fn apply_for(tool: &str, home: &str, p: &Provider) -> Result<(), AcsError> {
    match tool {
        "claude" => crate::claude::apply_provider(home, p),
        "codex" => crate::codex::apply_provider(home, p),
        "gemini" => crate::gemini::apply_provider(home, p),
        _ => unreachable!(),
    }
}

pub fn run_test(tool: &str, cfg: &mut AcsConfig) -> Result<(), AcsError> {
    let home = cfg.get_tool(tool).home.clone();
    let active = cfg.get_tool(tool).active.clone();
    let provider = match cfg.get_tool(tool).providers.get(&active) {
        Some(p) => p.clone(),
        None => return Err(ProviderError::no_providers(tool).into()),
    };

    let all_urls = provider.all_urls();
    if all_urls.is_empty() {
        return Err(InteractiveError::input(format!(
            "No URLs configured. Add one with `acs {} config --base-url <url>`",
            tool
        ))
        .into());
    }

    println!("Testing {} URL(s)...", all_urls.len());
    let results = probe_all(&all_urls);

    let primary = provider.base_url().to_string();
    let items: Vec<String> = results
        .iter()
        .map(|r| {
            let lat = match r.ms {
                Some(us) => format!("{:>7.1}ms", us as f64 / 1000.0).green().to_string(),
                None => "timeout".red().to_string(),
            };
            let tag = if r.url == primary { " *" } else { "" };
            format!("{}  {}{}", lat, r.url, tag)
        })
        .collect();

    let sel = match Select::new()
        .with_prompt("Select URL to apply as base URL")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|e| InteractiveError::input(e.to_string()))?
    {
        Some(i) => i,
        None => return Ok(()),
    };

    let chosen = &results[sel].url;
    if *chosen != primary {
        {
            let p = cfg.get_tool_mut(tool).providers.get_mut(&active).unwrap();
            p.set_base_url(tool, chosen.clone());
            let p_clone = p.clone();
            apply_for(tool, &home, &p_clone)?;
        }
        save_config(cfg)?;
        println!("Applied: {}", chosen);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn test_probe_tcp_unreachable() {
        assert!(probe_tcp("http://127.0.0.1:1").is_none());
    }

    #[test]
    fn test_probe_tcp_reachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(probe_tcp(&format!("http://127.0.0.1:{}", port)).is_some());
    }

    #[test]
    fn test_probe_all_sorts_by_latency() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let reachable = format!("http://127.0.0.1:{}", port);
        let results = probe_all(&["http://127.0.0.1:1".to_string(), reachable.clone()]);
        assert_eq!(results[0].url, reachable);
        assert!(results[0].ms.is_some());
        assert!(results[1].ms.is_none());
    }
}
