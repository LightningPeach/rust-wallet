//
// Copyright 2018 rust-wallet developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use structopt::StructOpt;
use std::path::PathBuf;
use wallet::mnemonic::Mnemonic;

#[derive(StructOpt, Debug)]
#[structopt(name = "wallet")]
/// Rust Wallet Config
pub struct Config {
    #[structopt(long="log-level", default_value="INFO")]
    /// should be one of ERROR, WARN, INFO, DEBUG, TRACE
    log_level: String,

    #[structopt(long="db-path", parse(from_os_str), default_value="target/db/wallet")]
    /// path to directory with wallet data
    db_path: PathBuf,

    #[structopt(long="rpc-port", default_value="5051")]
    /// port of wallet's grpc server
    rpc_port: u16,

    #[structopt(long="zmqpubrawblock", default_value="tcp://127.0.0.1:18501")]
    /// address of bitcoind's zmqpubrawblock endpoint
    /// relevant only if `bitcoind_uri` is not specified
    zmqpubrawblock: String,

    #[structopt(long="zmqpubrawtx", default_value="tcp://127.0.0.1:18501")]
    /// address of bitcoind's zmqpubrawtx endpoint
    /// relevant only if `bitcoind_uri` is not specified
    zmqpubrawtx: String,

    #[structopt(long="user")]
    /// bitcoind's rpc user
    user: String,

    #[structopt(long="password")]
    /// bitcoind's rpc password
    password: String,

    #[structopt(long="bitcoin-address")]
    /// address of bitcoind's rpc server, run bitcoind locally if not specified
    bitcoind_address: Option<String>,

    #[structopt(long="electrumx-address")]
    /// address of bitcoind's rpc server, run electrs locally if not specified
    /// relevant only if `electrumx` flag is set
    electrumx_address: Option<String>,

    #[structopt(long="electrumx")]
    /// create electrumx wallet
    electrumx: bool,

    #[structopt(long="mode", default_value="decrypt")]
    /// should be one of create|decrypt|recover
    mode: String,

    #[structopt(long="mnemonic")]
    /// relevant only `mode` is recover
    mnemonic: Option<String>,
}

fn main() {
    use rust_wallet_grpc::server;
    use std::str::FromStr;

    use wallet::{walletlibrary::{WalletLibraryMode, KeyGenConfig, DEFAULT_NETWORK}, context::GlobalContext};

    let config: Config = Config::from_args();

    let log_level = log::Level::from_str(config.log_level.as_str()).unwrap();
    simple_logger::init_with_level(log_level).unwrap();

    let context = GlobalContext::new(
        DEFAULT_NETWORK,
        config.user,
        config.password,
        Some(config.db_path.to_str().unwrap().to_owned()),
        config.bitcoind_address.as_ref().map(|s| s.parse().unwrap()),
        config.electrumx_address.as_ref().map(|s| s.parse().unwrap()),
    );

    // if `bitcoind_uri` is not specified run bitcoind locally
    let bitcoind = if config.bitcoind_address.is_none() {
        Some(context.bitcoind(config.zmqpubrawblock, config.zmqpubrawtx).unwrap())
    } else {
        None
    };

    // if `electrumx_uri` is not specified run electrs locally
    let electrs = if config.electrumx_address.is_none() {
        Some(context.electrs().unwrap())
    } else {
        None
    };

    let mode = if config.mode == "create" {
        WalletLibraryMode::Create(KeyGenConfig::default())
    } else if config.mode == "recover" {
        let mnemonic = config.mnemonic.unwrap();
        WalletLibraryMode::RecoverFromMnemonic(Mnemonic::from(mnemonic.trim_matches('"')).unwrap())
    } else {
        WalletLibraryMode::Decrypt
    };

    let (wallet_context, mnemonic) = if config.electrumx {
        context.electrs_context(mode).unwrap()
    } else {
        context.default_context(mode).unwrap()
    };
    println!("{}", mnemonic.to_string());

    let (wallet, _) = wallet_context.destruct();
    server::launch_server_new(wallet, config.rpc_port);

    if let Some(mut process) = electrs {
        log::info!("kill electrs");
        match process.kill() { _ => () }
    }
    if let Some(mut process) = bitcoind {
        log::info!("kill bitcoind");
        match process.kill() { _ => () }
    }
}
