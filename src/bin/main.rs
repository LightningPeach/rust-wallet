extern crate bitcoin;
extern crate hex;
extern crate wallet;
extern crate log;
extern crate simple_logger;
extern crate clap;

use clap::{Arg, App};

use std::str::FromStr;

use wallet::{
    server::{launch_server, DEFAULT_WALLET_RPC_PORT},
    accountfactory::{
        WalletConfig, BitcoindConfig,
        DEFAULT_NETWORK, DEFAULT_ENTROPY, DEFAULT_PASSPHRASE, DEFAULT_SALT, DEFAULT_DB_PATH,
        DEFAULT_BITCOIND_RPC_CONNECT, DEFAULT_BITCOIND_RPC_USER, DEFAULT_BITCOIND_RPC_PASSWORD,
        DEFAULT_ZMQ_PUB_RAW_BLOCK_ENDPOINT, DEFAULT_ZMQ_PUB_RAW_TX_ENDPOINT,
    },
};

fn main() {
    let default_wallet_rpc_port_str: &str = &DEFAULT_WALLET_RPC_PORT.to_string();

    let matches = App::new("wallet")
        .version("1.0")
        .arg(Arg::with_name("log_level")
            .long("log_level")
            .help("should be one of ERROR, WARN, INFO, DEBUG, TRACE")
            .takes_value(true)
            .default_value("INFO"))
        .arg(Arg::with_name("db_path")
            .long("db_path")
            .help("path to file with wallet data")
            .takes_value(true)
            .default_value(DEFAULT_DB_PATH))
        .arg(Arg::with_name("connect")
            .long("connect")
            .help("address of bitcoind's rpc server")
            .takes_value(true)
            .default_value(DEFAULT_BITCOIND_RPC_CONNECT))
        .arg(Arg::with_name("user")
            .long("user")
            .help("bitcoind's rpc user")
            .takes_value(true)
            .default_value(DEFAULT_BITCOIND_RPC_USER))
        .arg(Arg::with_name("password")
            .long("password")
            .help("bitcoind's rpc password")
            .takes_value(true)
            .default_value(DEFAULT_BITCOIND_RPC_PASSWORD))
        .arg(Arg::with_name("zmqpubrawblock")
            .long("zmqpubrawblock")
            .help("address of bitcoind's zmqpubrawblock endpoint")
            .takes_value(true)
            .default_value(DEFAULT_ZMQ_PUB_RAW_BLOCK_ENDPOINT))
        .arg(Arg::with_name("zmqpubrawtx")
            .long("zmqpubrawtx")
            .help("address of bitcoind's zmqpubrawtx endpoint")
            .takes_value(true)
            .default_value(DEFAULT_ZMQ_PUB_RAW_TX_ENDPOINT))
        .arg(Arg::with_name("wallet_rpc_port")
            .long("wallet_rpc_port")
            .help("port of wallet's grpc server")
            .takes_value(true)
            .default_value(default_wallet_rpc_port_str))
        .get_matches();

    let log_level = {
        let rez = matches.value_of("log_level").unwrap();
        let rez = log::Level::from_str(rez).unwrap();
        rez
    };
    simple_logger::init_with_level(log_level).unwrap();

    let wc = WalletConfig::new(
        DEFAULT_NETWORK,
        DEFAULT_ENTROPY,
        DEFAULT_PASSPHRASE.to_string(),
        DEFAULT_SALT.to_string(),
        matches.value_of("db_path").unwrap().to_string(),
    );

    let cfg = BitcoindConfig::new(
        matches.value_of("connect").unwrap().to_string(),
        matches.value_of("user").unwrap().to_string(),
        matches.value_of("password").unwrap().to_string(),
        matches.value_of("zmqpubrawblock").unwrap().to_string(),
        matches.value_of("zmqpubrawtx").unwrap().to_string(),
    );

    let wallet_rpc_port: u16 = matches.value_of("wallet_rpc_port").unwrap().parse().unwrap();
    launch_server(wc, cfg, wallet_rpc_port);
}