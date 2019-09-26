use wallet::context::GlobalContext;
use std::io::Result;

fn main() -> Result<()> {
    let context = GlobalContext::default();
    let mut bitcoind = context.bitcoind("tcp://127.0.0.1:18501".to_owned(), "tcp://127.0.0.1:18502".to_owned())?;
    let mut electrs = context.electrs()?;

    // TODO: use ctrlc crate to handle it properly
    electrs.wait().unwrap();
    bitcoind.wait().unwrap();

    Ok(())
}
