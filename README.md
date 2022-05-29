# RMDA-Example-Rust

A simple RDMA server&client example, using the `rdma-sys` ffi bindings.

1. TestCase

- Client post a receive work request, then server post a send work reqeust. When both side received a completion signal, the buffer in client side should be the content received from the server.

- Client use RDMA WRITE verbs to write buffer in server side, the buffer in server side should be the content written from the client.

2. Usage

in the server side
```bash
❯ cargo run -- -d rxe_0 -g 0 -i 1
    Finished dev [unoptimized + debuginfo] target(s) in 0.04s
     Running `target/debug/rdma-example-rust -d rxe_0 -g 0 -i 1`
Current Config: RdmaOpt { server: None, tcp_port: 18515, ib_devname: Some("rxe_0"), ib_port: 1, gidx: 0 }
May 29 22:12:35.145 DEBUG rdma_example_rust::context: Local Buffer addr: 0x5579265adf60
May 29 22:12:35.145 DEBUG rdma_example_rust::context: MR registered with addr=0x5579265adf60, lkey=0x4F01, rkey=0x4F01
May 29 22:12:35.145 DEBUG rdma_example_rust::context: QP was created, QP number=0x5E
May 29 22:12:35.145  INFO rdma_example_rust::context: current buf in RDMA context is: Hello World
May 29 22:12:35.145 DEBUG rdma_example_rust::context: Local Conn  0x5579265ADF60
May 29 22:12:35.145  INFO rdma_example_rust::context: Waiting for client to exchange the conn data
May 29 22:12:37.487 DEBUG rdma_example_rust::context: Remote Conn addr: 0x558B73A71080
May 29 22:12:37.487 DEBUG rdma_example_rust::context: INIT QP done
May 29 22:12:37.487 DEBUG rdma_example_rust::context: Modify QP to RTR state!
May 29 22:12:37.487 DEBUG rdma_example_rust::context: Modify QP to RTS state!
May 29 22:12:37.487 DEBUG rdma_example_rust::context: Send request was posted
May 29 22:12:37.488  INFO rdma_example_rust::context: current buf in RDMA context is: Hello World
May 29 22:12:37.488  INFO rdma_example_rust: the contents after RDMA WRITE in server/client's buffer: 
May 29 22:12:37.488  INFO rdma_example_rust::context: current buf in RDMA context is: CLIENT WANTS WRITE TO SERVER
```

in the client side
```bash
❯ cargo run -- 10.19.0.36  -d rxe_0 -g 0 -i 1
    Finished dev [unoptimized + debuginfo] target(s) in 0.04s
     Running `target/debug/rdma-example-rust 10.19.0.36 -d rxe_0 -g 0 -i 1`
Current Config: RdmaOpt { server: Some("10.19.0.36"), tcp_port: 18515, ib_devname: Some("rxe_0"), ib_port: 1, gidx: 0 }
May 29 22:12:37.486 DEBUG rdma_example_rust::context: Local Buffer addr: 0x558b73a71080
May 29 22:12:37.487 DEBUG rdma_example_rust::context: MR registered with addr=0x558b73a71080, lkey=0x5002, rkey=0x5002
May 29 22:12:37.487 DEBUG rdma_example_rust::context: QP was created, QP number=0x5F
May 29 22:12:37.487  INFO rdma_example_rust::context: current buf in RDMA context is: 
May 29 22:12:37.487 DEBUG rdma_example_rust::context: Local Conn  0x558B73A71080
May 29 22:12:37.487 DEBUG rdma_example_rust::context: Remote Conn addr: 0x5579265ADF60
May 29 22:12:37.487 DEBUG rdma_example_rust::context: INIT QP done
May 29 22:12:37.487 DEBUG rdma_example_rust::context: client post a receive work request
May 29 22:12:37.487 DEBUG rdma_example_rust::context: Receive request was posted
May 29 22:12:37.487 DEBUG rdma_example_rust::context: Modify QP to RTR state!
May 29 22:12:37.487 DEBUG rdma_example_rust::context: Modify QP to RTS state!
May 29 22:12:37.487  INFO rdma_example_rust::context: current buf in RDMA context is: Hello World
May 29 22:12:37.487 DEBUG rdma_example_rust::context: RDMA read request was posted
May 29 22:12:37.488  INFO rdma_example_rust: RDMA READ contents in server's buffer: 
May 29 22:12:37.488  INFO rdma_example_rust::context: current buf in RDMA context is: Hello World
May 29 22:12:37.488 DEBUG rdma_example_rust::context: buf addr: 0x558b73a71080
May 29 22:12:37.488 DEBUG rdma_example_rust::context: buf addr: 0x558b73a71080
May 29 22:12:37.488 DEBUG rdma_example_rust::context: RDMA write request was posted
May 29 22:12:37.488  INFO rdma_example_rust: the contents after RDMA WRITE in server/client's buffer: 
May 29 22:12:37.488  INFO rdma_example_rust::context: current buf in RDMA context is: CLIENT WANTS WRITE TO SERVER
```