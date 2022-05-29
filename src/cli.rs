use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author = "zelin", version = "0.0.1")]
#[clap(about = "the rdma example in rust")]
pub struct RdmaOpt {
    /// Optional server host. if it is none, the pingpong binary run as server
    pub(crate) server: Option<String>,
    /// listen on/connect to port <port> (default 18515)
    #[clap(short = 'p', long, default_value_t = 18515)]
    pub(crate) tcp_port: usize,
    /// use IB device <dev> (default first device found)
    #[clap(short = 'd', long)]
    pub(crate) ib_devname: Option<String>,
    /// use port <port> of IB device (default 1)
    #[clap(short = 'i', long, default_value_t = 1)]
    pub(crate) ib_port: u8,
    /// local port gid index
    #[clap(short = 'g', long, default_value_t = 0)]
    pub(crate) gidx: i32,
}
