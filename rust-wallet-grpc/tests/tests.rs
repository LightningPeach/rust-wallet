use bitcoin_rpc_client::Client;
use std::{thread, time::Duration, process::Child};
use rust_wallet_grpc::client::WalletClientWrapper;

const LAUNCH_SERVER_DELAY_MS: u64 = 3_000;
const SHUTDOWN_SERVER_DELAY_MS: u64 = 2_000;

fn run() -> (WalletClientWrapper, Client, Child) {
    use wallet::{context::GlobalContext, walletlibrary::{WalletLibraryMode, KeyGenConfig}};
    use rust_wallet_grpc::server;

    let context = GlobalContext::default();
    let bitcoind_process = context.bitcoind("tcp://127.0.0.1:18501".to_owned(), "tcp://127.0.0.1:18502".to_owned()).unwrap();

    let mode = WalletLibraryMode::Create(KeyGenConfig::default());

    let (wallet_context, _mnemonic) = context.default_context(mode).unwrap();
    let (wallet, bitcoin) = wallet_context.destruct();
    let _ = thread::spawn(move || server::launch_server_new(wallet, server::DEFAULT_WALLET_RPC_PORT));
    thread::sleep(Duration::from_millis(LAUNCH_SERVER_DELAY_MS));
    let wallet = WalletClientWrapper::new(server::DEFAULT_WALLET_RPC_PORT);

    (wallet, bitcoin, bitcoind_process)
}

fn shutdown(client: WalletClientWrapper, mut bitcoin_process: Child) {
    client.shutdown();
    bitcoin_process.kill().unwrap();
    thread::sleep(Duration::from_millis(SHUTDOWN_SERVER_DELAY_MS));
}

#[test]
fn basic() {
    use std::str::FromStr;
    use bitcoin::Address;
    use bitcoin_rpc_client::RpcApi;
    use rust_wallet_grpc::walletrpc::AddressType;

    let (wallet, bitcoin, bitcoin_process) = run();

    let address = {
        let a = wallet.new_address(AddressType::P2WKH);
        Address::from_str(a.as_str()).unwrap()
    };
    let _ = bitcoin.generate_to_address(1, &address).unwrap();
    wallet.sync_with_tip();
    let balance = wallet.wallet_balance();

    assert_eq!(balance, 50_0000_0000);

    let _ = bitcoin;
    shutdown(wallet, bitcoin_process);
}
