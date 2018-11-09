use bitcoin::Block;
use futures::{self, Poll, Async, Stream};

use std::sync::{
    atomic::AtomicBool,
    RwLock,
};

use accountfactory::AccountFactory;
use chainntfs::ZMQMessageProducer;

pub struct Wallet {
    pub backend: AccountFactory,
    blockchain_source: ZMQMessageProducer,
    shutdown: AtomicBool,
}

impl Stream for Wallet {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let poll_item = self.blockchain_source.poll().unwrap();
        match poll_item {
            Async::Ready(Some(block)) => {
                self.backend.process_wire_block(block);
                Ok(Async::Ready(Some(())))
            },
            Async::Ready(None) => {
                Ok(Async::Ready(None))
            }
            Async::NotReady => {
                futures::task::current().notify();
                Ok(Async::NotReady)
            }
        }
    }
}

impl Wallet {
    pub fn new(backend: AccountFactory, blockchain_source: ZMQMessageProducer) -> Self {
        Self {
            backend,
            blockchain_source,
            shutdown: AtomicBool::new(false),
        }
    }
}