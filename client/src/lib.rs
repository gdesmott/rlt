use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{self, AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};

pub const AD4M_PROXY_SERVER: &str = "http://proxy.ad4m.dev";
pub const LOCAL_HOST: &str = "127.0.0.1";

#[derive(Debug, Serialize, Deserialize)]
struct ProxyResponse {
    id: String,
    port: u16,
    max_conn_count: u8,
    url: String,
}

pub async fn open_tunnel(
    server: Option<&str>,
    subdomain: Option<&str>,
    local_host: Option<&str>,
    local_port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let server = server.unwrap_or(AD4M_PROXY_SERVER);
    let local_host = local_host.unwrap_or(LOCAL_HOST);
    println!("start connect to: {}, local port: {}", server, local_port);

    // Get custome domain
    let assigned_domain = subdomain.unwrap_or("?new");
    let uri = format!("{}/{}", server, assigned_domain);
    println!("assigned domain: {}", uri);
    let resp = reqwest::get(uri).await?.json::<ProxyResponse>().await?;
    println!("{:?}", resp);

    // Parse and get remote host
    let server_parts = server.split("//").collect::<Vec<&str>>();
    let server_host = server_parts[1];
    
    // TODO check the connect is failed and restart the proxy.

    let counter = Arc::new(Mutex::new(0));

    loop {
        sleep(Duration::from_millis(600)).await;

        let mut locked_counter = counter.lock().unwrap();
        if *locked_counter < resp.max_conn_count {
            println!("spawn new proxy");
            *locked_counter += 1;

            let server_host = server_host.to_string();
            let local_host = local_host.to_string();
            let counter2 = Arc::clone(&counter);
            tokio::spawn(async move {
                handle_conn(server_host, resp.port, local_host, local_port, counter2).await
            });
        }
    }
}

async fn handle_conn(remote_host: String, remote_port: u16, local_host: String, local_port: u16, counter: Arc<Mutex<u8>>) -> Result<()> {
    let remote_stream_in = TcpStream::connect(format!("{}:{}", remote_host, remote_port)).await?;
    let local_stream_in = TcpStream::connect(format!("{}:{}", local_host, local_port)).await?;

    proxy(remote_stream_in, local_stream_in, counter).await?;
    Ok(())
}

/// Copy data mutually between two read/write streams.
pub async fn proxy<S1, S2>(stream1: S1, stream2: S2, counter: Arc<Mutex<u8>>) -> io::Result<()>
where
    S1: AsyncRead + AsyncWrite + Unpin,
    S2: AsyncRead + AsyncWrite + Unpin,
{
    let (mut s1_read, mut s1_write) = io::split(stream1);
    let (mut s2_read, mut s2_write) = io::split(stream2);
    tokio::select! {
        res = io::copy(&mut s1_read, &mut s2_write) => res,
        res = io::copy(&mut s2_read, &mut s1_write) => res,
    }?;
    let mut locked_counter = counter.lock().unwrap();
    *locked_counter -= 1;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
