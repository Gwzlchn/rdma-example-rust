use clap::Parser;
mod cli;
mod context;
mod gid;
use cli::RdmaOpt;
use context::RdmaContext;
use rdma_sys::ibv_wr_opcode;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();
    let config = RdmaOpt::parse();
    println!("Current Config: {:?}", config);
    let mut rdma_context = RdmaContext::create(&config).unwrap();
    rdma_context.check_the_buf();

    rdma_context.connect_qp(&config).unwrap();
    // the server post the send request
    if config.server.is_none() {
        rdma_context.post_send(ibv_wr_opcode::IBV_WR_SEND).unwrap();
    }
    // in both sides we expect to get a completion
    // server: there's a send completion
    // client: there's a recv completion
    rdma_context.poll_completion().unwrap();
    rdma_context.check_the_buf();

    // Now the client performs an RDMA read and then write on server.
    if config.server.is_some() {
        rdma_context
            .post_send(ibv_wr_opcode::IBV_WR_RDMA_READ)
            .unwrap();
        rdma_context.poll_completion().unwrap();
        tracing::info!("RDMA READ contents in server's buffer: ");
        rdma_context.check_the_buf();
        rdma_context.set_str_to_buf("CLIENT WANTS WRITE TO SERVER");
        rdma_context
            .post_send(ibv_wr_opcode::IBV_WR_RDMA_WRITE)
            .unwrap();
        rdma_context.poll_completion().unwrap();
    }
    tracing::info!("the contents after RDMA WRITE in server/client's buffer: ");
    rdma_context.check_the_buf();
}
