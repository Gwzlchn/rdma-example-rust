use crate::cli::RdmaOpt;
use crate::gid::Gid;
use rdma_sys::*;
use serde::{Deserialize, Serialize};
use std::{
    ffi::CStr,
    io::{self, Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    str::FromStr,
};
use tracing::{debug, error, info, warn};

const TEST_STR: &str = "Hello World";
const BUFFER_SIZE: usize = 100; //1MB

/* structure to exchange data which is needed to connect the QPs */
#[derive(Default, Debug, Serialize, Deserialize)]
struct CmConData {
    addr: u64,   /* Buffer address */
    rkey: u32,   /* Remote key */
    qp_num: u32, /* QP number */
    lid: u16,    /* LID of the IB port */
    gid: Gid,    /* gid */
}

pub struct RdmaContext {
    port_attr: ibv_port_attr,
    remote_props: CmConData,
    ib_ctx: *mut ibv_context,
    pd: *mut ibv_pd,
    cq: *mut ibv_cq,
    qp: *mut ibv_qp,
    mr: *mut ibv_mr,
    buf: Vec<u8>,
}

impl RdmaContext {
    pub fn create(config: &RdmaOpt) -> io::Result<Self> {
        // get dev
        let mut num_devs: i32 = 0;
        let dev_list_ptr = unsafe { ibv_get_device_list(&mut num_devs) };
        let dev_list = unsafe { std::slice::from_raw_parts(dev_list_ptr, num_devs as _) };
        let dev = if let Some(dev_name_inner) = &config.ib_devname {
            dev_list
                .iter()
                .find(|iter_dev| -> bool {
                    let name = unsafe { ibv_get_device_name(**iter_dev) };
                    assert!(!name.is_null());
                    let name = unsafe { CStr::from_ptr(name) }.to_str();
                    if name.is_err() {
                        warn!("Device name {:?} is not valid", name);
                        return false;
                    }

                    // We've checked the name is not error
                    #[allow(clippy::unwrap_used)]
                    let name = name.unwrap();
                    dev_name_inner.eq(name)
                })
                .ok_or(io::ErrorKind::NotFound)?
        } else {
            dev_list.get(0).ok_or(io::ErrorKind::NotFound)?
        };
        // open the dev
        let ib_ctx = unsafe { ibv_open_device(*dev) };
        assert!(!ib_ctx.is_null());
        // free the list ptr
        unsafe { ibv_free_device_list(dev_list_ptr) };
        let mut port_attr = unsafe { std::mem::zeroed() };
        // query port properties
        unsafe {
            ___ibv_query_port(ib_ctx, config.ib_port, &mut port_attr);
        }
        // alloc pd
        let pd = unsafe { ibv_alloc_pd(ib_ctx) };
        assert!(!pd.is_null());
        // create cq
        let cq_size = 1;
        let cq = unsafe {
            ibv_create_cq(
                ib_ctx,
                cq_size,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        };
        assert!(!cq.is_null());
        // create buffer
        // if it is server, use the TEST_STR,
        // it it is client, use the empty buffer
        let mut buf = if config.server.is_none() {
            let mut str_buf = TEST_STR.as_bytes().to_vec();
            str_buf.resize(BUFFER_SIZE, 0);
            str_buf
        } else {
            vec![0; BUFFER_SIZE]
        };
        debug!("Local Buffer addr: {:p}", buf.as_ptr());
        // register the memory buffer
        let mr_access_flags = ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_READ
            | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE;
        let mr = unsafe {
            ibv_reg_mr(
                pd,
                buf.as_mut_ptr() as *mut _,
                BUFFER_SIZE,
                mr_access_flags.0 as i32,
            )
        };
        debug!(
            "MR registered with addr={:p}, lkey={:#0X}, rkey={:#0X}",
            buf.as_mut_ptr(),
            unsafe { (*mr).lkey },
            unsafe { (*mr).rkey }
        );
        // create qp
        let mut qp_init_attr = unsafe { std::mem::zeroed::<ibv_qp_init_attr>() };
        qp_init_attr.qp_type = ibv_qp_type::IBV_QPT_RC;
        qp_init_attr.sq_sig_all = 1;
        qp_init_attr.send_cq = cq;
        qp_init_attr.recv_cq = cq;
        qp_init_attr.cap.max_send_wr = 1;
        qp_init_attr.cap.max_recv_wr = 1;
        qp_init_attr.cap.max_send_sge = 1;
        qp_init_attr.cap.max_recv_sge = 1;
        let qp = unsafe { ibv_create_qp(pd, &mut qp_init_attr) };
        debug!("QP was created, QP number={:#0X}", unsafe { (*qp).qp_num });
        Ok(RdmaContext {
            port_attr,
            remote_props: Default::default(), // it will set in connect_qp
            ib_ctx,
            pd,
            cq,
            qp,
            mr,
            buf,
        })
    }
    // connect the qp , transfer the state in server side to RTR, the state in client side to RTS
    pub fn connect_qp(&mut self, config: &RdmaOpt) -> Result<(), io::Error> {
        // set local host
        let mut local_gid = Gid::default();
        if config.gidx >= 0 {
            unsafe { ibv_query_gid(self.ib_ctx, config.ib_port, config.gidx, local_gid.ffi()) };
        }
        let local_con_data = CmConData {
            addr: self.buf.as_ptr() as _,
            rkey: unsafe { (*self.mr).rkey },
            qp_num: unsafe { (*self.qp).qp_num },
            lid: self.port_attr.lid,
            gid: local_gid,
        };
        debug!("Local Conn  {:#0X}", local_con_data.addr);
        let local_con_data_encoded = bincode::serialize(&local_con_data).unwrap();
        let mut temp_con_data_encoded = bincode::serialize(&CmConData::default()).unwrap();
        // if it is server
        if config.server.is_none() {
            let server_socket =
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), config.tcp_port as _);
            let listener = TcpListener::bind(server_socket).unwrap();
            info!("Waiting for client to exchange the conn data");
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        stream.write_all(&local_con_data_encoded).unwrap();
                        stream.read_exact(&mut temp_con_data_encoded).unwrap();
                        break;
                    }
                    Err(_) => {
                        error!("Error in connect qp")
                    }
                }
            }
        } else {
            // if it is client
            let client_socket = SocketAddr::new(
                IpAddr::from_str(config.server.as_ref().unwrap()).unwrap(),
                config.tcp_port as _,
            );
            let mut stream = TcpStream::connect(client_socket).unwrap();
            stream.write_all(&local_con_data_encoded).unwrap();
            stream.read_exact(&mut temp_con_data_encoded).unwrap();
        }
        self.remote_props = bincode::deserialize(&temp_con_data_encoded).unwrap();
        debug!("Remote Conn addr: {:#0X}", self.remote_props.addr);

        self.modify_qp_to_init(config.ib_port)?;
        if config.server.is_some() {
            debug!("client post a receive work request");
            self.post_receive()?;
        }
        self.modify_qp_to_rtr(
            config,
            self.remote_props.qp_num,
            self.remote_props.lid,
            self.remote_props.gid,
        )?;
        self.modify_qp_to_rts()?;
        Ok(())
    }
    pub fn modify_qp_to_init(&self, ib_port: u8) -> Result<(), io::Error> {
        let mut qp_attr = unsafe { std::mem::zeroed::<ibv_qp_attr>() };
        qp_attr.qp_state = ibv_qp_state::IBV_QPS_INIT;
        qp_attr.port_num = ib_port;
        qp_attr.pkey_index = 0;
        qp_attr.qp_access_flags = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_READ
            | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE)
            .0;
        let attr_mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_PKEY_INDEX
            | ibv_qp_attr_mask::IBV_QP_PORT
            | ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS;
        let err =
            unsafe { ibv_modify_qp(self.qp, &mut qp_attr, (attr_mask.0).try_into().unwrap()) };
        if err == 0 {
            debug!("INIT QP done");
            Ok(())
        } else {
            error!("INIT QP got an error");
            Err(io::Error::from_raw_os_error(err))
        }
    }

    pub fn modify_qp_to_rtr(
        &self,
        config: &RdmaOpt,
        remote_qpn: u32,
        dlid: u16,
        dgid: Gid,
    ) -> Result<(), io::Error> {
        let mut qp_attr = unsafe { std::mem::zeroed::<ibv_qp_attr>() };
        qp_attr.qp_state = ibv_qp_state::IBV_QPS_RTR;
        qp_attr.path_mtu = ibv_mtu::IBV_MTU_1024;
        qp_attr.dest_qp_num = remote_qpn;
        qp_attr.rq_psn = 0;
        qp_attr.max_dest_rd_atomic = 1;
        qp_attr.min_rnr_timer = 0x12;
        qp_attr.ah_attr.is_global = 0;
        qp_attr.ah_attr.dlid = dlid;
        qp_attr.ah_attr.sl = 0;
        qp_attr.ah_attr.src_path_bits = 0;
        qp_attr.ah_attr.port_num = config.ib_port;
        if config.gidx >= 0 {
            qp_attr.ah_attr.is_global = 1;
            qp_attr.ah_attr.port_num = 1;
            qp_attr.ah_attr.grh.dgid = dgid.into();
            qp_attr.ah_attr.grh.flow_label = 0;
            qp_attr.ah_attr.grh.hop_limit = 1;
            qp_attr.ah_attr.grh.sgid_index = config.gidx as _;
            qp_attr.ah_attr.grh.traffic_class = 0;
        }
        let attr_mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_AV
            | ibv_qp_attr_mask::IBV_QP_PATH_MTU
            | ibv_qp_attr_mask::IBV_QP_DEST_QPN
            | ibv_qp_attr_mask::IBV_QP_RQ_PSN
            | ibv_qp_attr_mask::IBV_QP_MAX_DEST_RD_ATOMIC
            | ibv_qp_attr_mask::IBV_QP_MIN_RNR_TIMER;

        let err = unsafe { ibv_modify_qp(self.qp, &mut qp_attr, (attr_mask.0) as _) };
        if err == 0 {
            debug!("Modify QP to RTR state!");
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(err))
        }
    }
    pub fn modify_qp_to_rts(&self) -> Result<(), io::Error> {
        let mut qp_attr = unsafe { std::mem::zeroed::<ibv_qp_attr>() };

        qp_attr.qp_state = ibv_qp_state::IBV_QPS_RTS;
        qp_attr.timeout = 0x12; // 18
        qp_attr.retry_cnt = 6;
        qp_attr.rnr_retry = 0;
        qp_attr.sq_psn = 0;
        qp_attr.max_rd_atomic = 1;
        let attr_mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_TIMEOUT
            | ibv_qp_attr_mask::IBV_QP_RETRY_CNT
            | ibv_qp_attr_mask::IBV_QP_RNR_RETRY
            | ibv_qp_attr_mask::IBV_QP_SQ_PSN
            | ibv_qp_attr_mask::IBV_QP_MAX_QP_RD_ATOMIC;

        let err = unsafe { ibv_modify_qp(self.qp, &mut qp_attr, (attr_mask.0) as _) };
        if err == 0 {
            debug!("Modify QP to RTS state!");
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(err))
        }
    }

    // create and send a work request
    pub fn post_send(&mut self, opcode: ibv_wr_opcode::Type) -> Result<(), io::Error> {
        let mut sge = unsafe { std::mem::zeroed::<ibv_sge>() };
        sge.addr = self.buf.as_mut_ptr() as _;
        sge.length = BUFFER_SIZE as _;
        sge.lkey = unsafe { (*self.mr).lkey };

        let mut send_wr = unsafe { std::mem::zeroed::<ibv_send_wr>() };
        send_wr.next = std::ptr::null_mut();
        send_wr.wr_id = 0;
        send_wr.sg_list = &mut sge;
        send_wr.num_sge = 1;
        send_wr.opcode = opcode;
        send_wr.send_flags = (ibv_send_flags::IBV_SEND_SIGNALED).0;

        if opcode != ibv_wr_opcode::IBV_WR_SEND {
            send_wr.wr.rdma.remote_addr = self.remote_props.addr;
            send_wr.wr.rdma.rkey = self.remote_props.rkey;
        }
        let mut bad_wr: *mut ibv_send_wr = std::ptr::null_mut();
        let err = unsafe { ibv_post_send(self.qp, &mut send_wr, &mut bad_wr) };
        if err == 0 {
            match opcode {
                ibv_wr_opcode::IBV_WR_SEND => debug!("Send request was posted"),
                ibv_wr_opcode::IBV_WR_RDMA_READ => debug!("RDMA read request was posted"),
                ibv_wr_opcode::IBV_WR_RDMA_WRITE => debug!("RDMA write request was posted"),
                _ => debug!("Unknown request was posted"),
            }
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(err))
        }
    }
    pub fn post_receive(&mut self) -> Result<(), io::Error> {
        let mut sge = unsafe { std::mem::zeroed::<ibv_sge>() };
        sge.addr = self.buf.as_mut_ptr() as _;
        sge.length = BUFFER_SIZE as _;
        sge.lkey = unsafe { (*self.mr).lkey };

        let mut recv_wr = unsafe { std::mem::zeroed::<ibv_recv_wr>() };
        recv_wr.next = std::ptr::null_mut();
        recv_wr.wr_id = 100;
        recv_wr.sg_list = &mut sge;
        recv_wr.num_sge = 1;

        let mut bad_wr: *mut ibv_recv_wr = std::ptr::null_mut();
        let err = unsafe { ibv_post_recv(self.qp, &mut recv_wr, &mut bad_wr) };
        if err == 0 {
            debug!("Receive request was posted");
            Ok(())
        } else {
            error!("Receive request posted got an error, Wr ID: {}", unsafe {
                (*bad_wr).wr_id
            });
            Err(io::Error::from_raw_os_error(err))
        }
    }

    pub fn poll_completion(&self) -> Result<(), io::Error> {
        let mut wc = unsafe { std::mem::zeroed::<ibv_wc>() };
        let mut poll_result: i32;
        loop {
            poll_result = unsafe { ibv_poll_cq(self.cq, 1, &mut wc) };
            if poll_result != 0 {
                break;
            }
        }
        if poll_result < 0 {
            error!("Poll CQ failed");
            Err(io::Error::from_raw_os_error(poll_result))
        } else if wc.status != ibv_wc_status::IBV_WC_SUCCESS {
            error!(
                "got bad completion with status: {:#0X}, vendor syndrome: {:#0X}",
                wc.status, wc.vendor_err
            );
            Err(io::Error::new(io::ErrorKind::InvalidData, "WC Failed"))
        } else {
            Ok(())
        }
    }
    pub fn set_str_to_buf(&mut self, new_str: &str) {
        debug!("buf addr: {:p}", self.buf.as_ptr());
        let mut new_str_bytes = new_str.as_bytes().to_vec();
        new_str_bytes.resize(BUFFER_SIZE, 0);
        self.buf.copy_from_slice(&new_str_bytes);
        debug!("buf addr: {:p}", self.buf.as_ptr());
    }
    pub fn check_the_buf(&self) {
        info!(
            "current buf in RDMA context is: {}",
            std::str::from_utf8(&self.buf).unwrap()
        );
    }
}

impl Drop for RdmaContext {
    fn drop(&mut self) {
        let mut err = unsafe { ibv_destroy_qp(self.qp) };
        assert_eq!(err, 0);
        err = unsafe { ibv_dereg_mr(self.mr) };
        assert_eq!(err, 0);
        err = unsafe { ibv_destroy_cq(self.cq) };
        assert_eq!(err, 0);
        err = unsafe { ibv_dealloc_pd(self.pd) };
        assert_eq!(err, 0);
        err = unsafe { ibv_close_device(self.ib_ctx) };
        assert_eq!(err, 0);
    }
}
