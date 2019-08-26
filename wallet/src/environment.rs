use wallet::context::GlobalContext;
use std::io::Result;

fn main() -> Result<()> {
    let context = GlobalContext::default();
    let mut bitcoind = context.bitcoind(18501, 18502)?;
    let mut electrs = context.electrs()?;

    // TODO: use ctrlc crate to handle it properly
    electrs.wait().unwrap();
    bitcoind.wait().unwrap();

    Ok(())
}
