use anyhow::Result;
use blink_timer::{IpcRequest, IpcResponse, async_socket::SocketStream};
use clap::{Parser, command};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();
    let args = Args::parse();
    Client::connect().await?.run(args.cmd).await?;
    Ok(())
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Get status of current timers
    Status,
    /// Toggle the timer
    Toggle,
    /// Reset all timers
    Reset,
}

struct Client {
    stream: SocketStream,
}

impl Client {
    async fn connect() -> Result<Self> {
        let stream = SocketStream::connect(blink_timer::socket_path()).await?;
        Ok(Self { stream })
    }

    async fn run(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::Status => {
                if let IpcResponse::Status(status) =
                    self.stream.send_and_recv(IpcRequest::Status).await?
                {
                    println!("{status}");
                }
            }
            Command::Toggle => {
                if !matches!(
                    self.stream.send_and_recv(IpcRequest::Toggle).await?,
                    IpcResponse::Ok
                ) {
                    eprintln!("unexpected response from deaemon");
                }
            }
            Command::Reset => {
                if !matches!(
                    self.stream.send_and_recv(IpcRequest::Reset).await?,
                    IpcResponse::Ok
                ) {
                    eprintln!("unexpected response from deaemon");
                }
            }
        };
        Ok(())
    }
}
