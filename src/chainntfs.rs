use bitcoin::{
    network::serialize::deserialize,
    Block,
};
use zmq;
use futures::{self, Poll, Async, Stream};

use std::sync::mpsc::Receiver;

pub struct ZMQMessageProducer {
    socket: zmq::Socket,
}

impl ZMQMessageProducer {
    pub fn new(zmq_addr: &str) -> Self {
        println!("connecting to bitcoind's server...");
        let context = zmq::Context::new();
        let socket = context.socket(zmq::SUB).unwrap();
        socket.set_subscribe(b"rawblock").unwrap();
        assert!(socket.connect(zmq_addr).is_ok());
        Self { socket }
    }
}

impl Stream for ZMQMessageProducer {
    type Item = Block;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let poll_item = self.socket.as_poll_item(zmq::POLLIN);
        match zmq::poll(&mut [poll_item], 0).unwrap() {
            0 => {
                futures::task::current().notify();
                Ok(Async::NotReady)
            },
            _ => {
                let msg_type = self.socket.recv_string(0).unwrap().unwrap();
                let bytes = self.socket.recv_bytes(0).unwrap();
                let block: Block = deserialize(&bytes).unwrap();
                self.socket.recv_string(0).unwrap().unwrap().as_str();
                Ok(Async::Ready(Some(block)))
            }
        }
    }
}