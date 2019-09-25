use super::{
    interface::Wallet,
    default::WalletWithTrustedFullNode,
    electrumx::ElectrumxWallet,
    walletlibrary::WalletConfig,
    walletlibrary::WalletLibraryMode,
    mnemonic::Mnemonic,
};
use bitcoin_rpc_client::{Client, Auth, Error as BitcoinError};
use std::{process::{Child, Command}, error::Error, io};
use bitcoin::network::constants::Network;

pub struct GlobalContext {
    network: Network,
    bitcoind_url: String,
    bitcoind_auth: Auth,
    port: u16,
    db_path: String,
    cookie: String,
    wallet_config: WalletConfig,
}

impl Default for GlobalContext {
    fn default() -> Self {
        let user = "devuser".to_owned();
        let password = "devpass".to_owned();
        GlobalContext::new(Network::Regtest, 18443, user, password)
    }
}

impl GlobalContext {
    pub fn new(network: Network, rpc_port: u16, user: String, password: String) -> Self {
        use super::walletlibrary::WalletConfigBuilder;
        use std::time::{SystemTime, UNIX_EPOCH};

        let url = format!("http://127.0.0.1:{}", rpc_port);
        let auth = Auth::UserPass(user.clone(), password.clone());

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let db_path = format!("/tmp/test_{:?}", now.as_secs());
        let config = WalletConfigBuilder::new()
            .network(network.clone())
            .db_path(db_path.clone())
            .finalize();

        GlobalContext {
            network: network,
            bitcoind_url: url,
            bitcoind_auth: auth,
            port: rpc_port,
            cookie: format!("{}:{}", user, password),
            db_path: db_path,
            wallet_config: config,
        }
    }

    pub fn bitcoind(&self, zmqpubrawblock_port: u16, zmqpubrawtx_port: u16) -> Result<Child, io::Error> {
        use std::{thread, time::Duration};
        use bitcoin_rpc_client::RpcApi;

        let auth_args = match &self.bitcoind_auth {
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
            .arg(format!("-rpcport={}", self.port))
            .arg(format!("-zmqpubrawblock=tcp://127.0.0.1:{}", zmqpubrawblock_port))
            .arg(format!("-zmqpubrawtx=tcp://127.0.0.1:{}", zmqpubrawtx_port))
            .spawn()?;
        thread::sleep(Duration::from_millis(2_000));

        let _ = self.client().unwrap().generate(1, None).unwrap();

        Ok(r)
    }

    pub fn electrs(&self) -> Result<Child, io::Error> {
        use std::{thread, time::Duration};

        const LAUNCH_ELECTRUMX_SERVER_DELAY_MS: u64 = 500;

        let electrs_process = Command::new("electrs")
            .arg("--jsonrpc-import")
            .arg(format!("--cookie={}", self.cookie))
            .arg(format!("--daemon-rpc-addr=127.0.0.1:{}", self.port))
            .arg(format!("--network={}", self.network))
            .arg(format!("--db-dir={}", self.db_path))
            .spawn();
        thread::sleep(Duration::from_millis(LAUNCH_ELECTRUMX_SERVER_DELAY_MS));
        electrs_process
    }

    fn client(&self) -> Result<Client, BitcoinError> {
        Client::new(self.bitcoind_url.clone(), self.bitcoind_auth.clone())
    }

    pub fn default_context(&self, mode: WalletLibraryMode) -> Result<(WalletContext, Mnemonic), Box<dyn Error>> {
        let cfg = self.wallet_config.clone();
        let (wallet, mnemonic) = WalletWithTrustedFullNode::new(cfg, self.client()?, mode)?;
        Ok((WalletContext::Default {
            wallet: Box::new(wallet),
            bitcoind: self.client()?,
        }, mnemonic))
    }

    pub fn electrs_context(&self, mode: WalletLibraryMode) -> Result<(WalletContext, Mnemonic), Box<dyn Error>> {
        let cfg = self.wallet_config.clone();
        let (wallet, mnemonic) = ElectrumxWallet::new(cfg, mode)?;
        Ok((WalletContext::Electrs {
            wallet: Box::new(wallet),
            bitcoind: self.client()?,
        }, mnemonic))
    }
}

pub enum WalletContext {
    Default {
        wallet: Box<dyn Wallet>,
        bitcoind: Client,
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
                bitcoind: _,
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
                bitcoind: ref mut r,
            } => r,
            &mut WalletContext::Electrs {
                wallet: _,
                bitcoind: ref mut r,
            } => r,
        }
    }
}
