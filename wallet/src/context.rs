use super::{
    interface::Wallet,
    default::WalletWithTrustedFullNode,
    electrumx::ElectrumxWallet,
    walletlibrary::WalletConfig,
    walletlibrary::WalletLibraryMode,
    mnemonic::Mnemonic,
};
use bitcoin_rpc_client::{Client, Auth, Error as BitcoinError};
use std::{process::{Child, Command}, error::Error, io, net::SocketAddr};
use bitcoin::network::constants::Network;

pub struct GlobalContext {
    network: Network,
    bitcoin_auth: Auth,
    bitcoin_socket_address: SocketAddr,
    electrum_auth: String,
    electrum_socket_address: Option<SocketAddr>,
    db_path: String,
    wallet_config: WalletConfig,
}

impl Default for GlobalContext {
    fn default() -> Self {
        let user = "devuser".to_owned();
        let password = "devpass".to_owned();
        GlobalContext::new(Network::Regtest, user, password, None, None, None)
    }
}

impl GlobalContext {
    pub fn new(
        network: Network,
        user: String,
        password: String,
        db_path: Option<String>,
        bitcoin_socket_address: Option<SocketAddr>,
        electrum_socket_address: Option<SocketAddr>,
    ) -> Self {
        use super::walletlibrary::WalletConfigBuilder;
        use std::time::{SystemTime, UNIX_EPOCH};

        let bitcoin_socket_address = bitcoin_socket_address.unwrap_or("127.0.0.1:18443".parse().unwrap());
        let auth = Auth::UserPass(user.clone(), password.clone());

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let db_path = db_path.unwrap_or(format!("/tmp/test_{:?}", now.as_secs()));
        let config = WalletConfigBuilder::new()
            .network(network.clone())
            .db_path(db_path.clone())
            .finalize();

        GlobalContext {
            network: network,
            bitcoin_auth: auth,
            bitcoin_socket_address: bitcoin_socket_address,
            electrum_auth: format!("{}:{}", user, password),
            electrum_socket_address: electrum_socket_address,
            db_path: db_path,
            wallet_config: config,
        }
    }

    pub fn bitcoind(&self, zmqpubrawblock: String, zmqpubrawtx: String) -> Result<Child, io::Error> {
        use std::{thread, time::Duration};
        use bitcoin_rpc_client::RpcApi;

        assert!(self.bitcoin_socket_address.ip().is_loopback());

        let auth_args = match &self.bitcoin_auth {
            &Auth::None => vec![],
            &Auth::CookieFile(_) => vec![],
            &Auth::UserPass(ref user, ref password) => vec![
                format!("-rpcuser={}", user.clone()),
                format!("-rpcpassword={}", password.clone()),
            ],
        };

        let r = Command::new("bitcoind")
            .args(&["-deprecatedrpc=generate"])
            .args(auth_args)
            .arg(format!("-{}", self.network.clone()))
            .arg(format!("-txindex"))
            .arg(format!("-rpcport={}", self.bitcoin_socket_address.port()))
            .arg(format!("-zmqpubrawblock={}", zmqpubrawblock))
            .arg(format!("-zmqpubrawtx={}", zmqpubrawtx))
            .spawn()?;
        thread::sleep(Duration::from_millis(2_000));

        let _ = self.client().unwrap().generate(1, None).unwrap();

        Ok(r)
    }

    pub fn electrs(&self) -> Result<Child, io::Error> {
        use std::{thread, time::Duration};

        const LAUNCH_ELECTRUMX_SERVER_DELAY_MS: u64 = 500;

        if let Some(ref address) = self.electrum_socket_address {
            assert!(address.ip().is_loopback());
        }

        let electrs_process = Command::new("electrs")
            .arg("--jsonrpc-import")
            .arg(format!("--cookie={}", self.electrum_auth))
            .arg(format!("--daemon-rpc-addr={}", self.bitcoin_socket_address))
            .arg(format!("--network={}", self.network))
            .arg(format!("--db-dir={}", self.db_path))
            .args(self.electrum_socket_address.iter().map(|&address| format!("--electrum-rpc-addr={}", address)))
            .spawn();
        thread::sleep(Duration::from_millis(LAUNCH_ELECTRUMX_SERVER_DELAY_MS));
        electrs_process
    }

    fn client(&self) -> Result<Client, BitcoinError> {
        let url = format!("http://{}", self.bitcoin_socket_address);
        Client::new(url, self.bitcoin_auth.clone())
    }

    pub fn default_context(&self, mode: WalletLibraryMode) -> Result<(WalletContext, Mnemonic), Box<dyn Error>> {
        let cfg = self.wallet_config.clone();
        let (wallet, mnemonic) = WalletWithTrustedFullNode::new(cfg, self.client()?, mode)?;
        Ok((WalletContext::Default {
            wallet: Box::new(wallet),
            bitcoin: self.client()?,
        }, mnemonic))
    }

    pub fn electrs_context(&self, mode: WalletLibraryMode) -> Result<(WalletContext, Mnemonic), Box<dyn Error>> {
        let cfg = self.wallet_config.clone();

        let default_electrum_rpc_port = match self.network {
            Network::Bitcoin => 50001,
            Network::Testnet => 60001,
            Network::Regtest => 60401,
        };
        let default_electrum_socket_address = format!("127.0.0.1:{}", default_electrum_rpc_port).parse().unwrap();
        let electrum_socket_address = self.electrum_socket_address.unwrap_or(default_electrum_socket_address);

        let (wallet, mnemonic) = ElectrumxWallet::new(electrum_socket_address, cfg, mode)?;
        Ok((WalletContext::Electrs {
            wallet: Box::new(wallet),
            bitcoind: self.client()?,
        }, mnemonic))
    }
}

pub enum WalletContext {
    Default {
        wallet: Box<dyn Wallet>,
        bitcoin: Client,
    },
    Electrs {
        wallet: Box<dyn Wallet>,
        bitcoind: Client,
    }
}

impl WalletContext {
    pub fn block_for_sync(&self) {
        use std::{thread, time::Duration};

        // TODO: poll event instead
        const ELECTRUMX_SERVER_SYNC_WITH_BLOCKCHAIN_DELAY_MS: u64 = 6_000;

        match self {
            &WalletContext::Default { .. } => (),
            &WalletContext::Electrs { .. } => {
                thread::sleep(Duration::from_millis(ELECTRUMX_SERVER_SYNC_WITH_BLOCKCHAIN_DELAY_MS));
            }
        }
    }

    pub fn wallet_mut(&mut self) -> &mut Box<dyn Wallet> {
        match self {
            &mut WalletContext::Default {
                wallet: ref mut r,
                bitcoin: _,
            } => r,
            &mut WalletContext::Electrs {
                wallet: ref mut r,
                bitcoind: _,
            } => r,
        }
    }

    pub fn bitcoind_mut(&mut self) -> &mut Client {
        match self {
            &mut WalletContext::Default {
                wallet: _,
                bitcoin: ref mut r,
            } => r,
            &mut WalletContext::Electrs {
                wallet: _,
                bitcoind: ref mut r,
            } => r,
        }
    }
}
