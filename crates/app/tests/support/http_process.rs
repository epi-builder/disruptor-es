use std::{
    env,
    io::Read,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, anyhow};
use reqwest::{Client, Method, StatusCode, header::HeaderMap};
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use tokio::time::sleep;

pub struct HttpResponse {
    pub status: StatusCode,
    #[allow(dead_code)]
    pub headers: HeaderMap,
    pub body: String,
}

pub struct AppProcess {
    _container: ContainerAsync<Postgres>,
    child: Child,
    pub listen_addr: SocketAddr,
    client: Client,
}

impl Drop for AppProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub async fn spawn_app() -> anyhow::Result<AppProcess> {
    let container = Postgres::default().with_tag("18").start().await?;
    let port = container.get_host_port_ipv4(5432).await?;
    let database_url =
        format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres?sslmode=disable");
    let listen_addr = free_listen_addr()?;
    let binary = app_binary()?;
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("building external-process test HTTP client")?;

    let child = Command::new(binary)
        .arg("serve")
        .env("DATABASE_URL", &database_url)
        .env("APP_LISTEN_ADDR", listen_addr.to_string())
        .env("APP_LOG_FILTER", "warn")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting app serve child process")?;

    let mut process = AppProcess {
        _container: container,
        child,
        listen_addr,
        client,
    };
    wait_for_health(&mut process).await?;
    Ok(process)
}

pub async fn wait_for_health(process: &mut AppProcess) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = process
            .child
            .try_wait()
            .context("checking serve child status")?
        {
            return Err(anyhow!(
                "app serve exited before readiness with {status}.{}",
                child_logs(&mut process.child)
            ));
        }

        match http_request(
            &process.client,
            process.listen_addr,
            Method::GET,
            "/healthz",
            Option::<&()>::None,
        )
        .await
        {
            Ok(response) if response.status == StatusCode::OK && response.body.trim() == "ok" => {
                return Ok(());
            }
            Ok(_) | Err(_) if Instant::now() < deadline => sleep(Duration::from_millis(150)).await,
            Ok(response) => {
                return Err(anyhow!(
                    "health probe never became ready: {}",
                    response.body
                ));
            }
            Err(error) => {
                return Err(error).context("health probe never became ready before timeout");
            }
        }
    }
}

pub async fn http_request<T: serde::Serialize + ?Sized>(
    client: &Client,
    addr: SocketAddr,
    method: Method,
    path: &str,
    body: Option<&T>,
) -> anyhow::Result<HttpResponse> {
    let url = format!("http://{addr}{path}");
    let builder = client.request(method, url);
    let builder = match body {
        Some(body) => builder.json(body),
        None => builder,
    };
    let response = builder
        .send()
        .await
        .with_context(|| format!("sending HTTP request to {path}"))?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .text()
        .await
        .context("reading HTTP response body")?;

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

pub fn child_logs(child: &mut Child) -> String {
    let mut combined = String::new();
    if let Some(stdout) = child.stdout.as_mut() {
        let mut buf = String::new();
        let _ = stdout.read_to_string(&mut buf);
        if !buf.is_empty() {
            combined.push_str("\n--- stdout ---\n");
            combined.push_str(&buf);
        }
    }
    if let Some(stderr) = child.stderr.as_mut() {
        let mut buf = String::new();
        let _ = stderr.read_to_string(&mut buf);
        if !buf.is_empty() {
            combined.push_str("\n--- stderr ---\n");
            combined.push_str(&buf);
        }
    }
    combined
}

impl AppProcess {
    pub async fn request<T: serde::Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: Option<&T>,
    ) -> anyhow::Result<HttpResponse> {
        http_request(&self.client, self.listen_addr, method, path, body).await
    }
}

fn free_listen_addr() -> anyhow::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0").context("binding ephemeral listen port")?;
    let addr = listener
        .local_addr()
        .context("reading ephemeral listen port")?;
    drop(listener);
    Ok(addr)
}

fn app_binary() -> anyhow::Result<PathBuf> {
    if let Ok(path) = env::var("CARGO_BIN_EXE_app") {
        return Ok(path.into());
    }

    let current_exe = env::current_exe().context("locating current test executable")?;
    let debug_dir = current_exe
        .parent()
        .and_then(|deps| deps.parent())
        .context("resolving target/debug directory from test executable")?;
    Ok(debug_dir.join("app"))
}
