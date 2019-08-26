extern crate bitcoin_rpc_client;
extern crate bitcoin;
extern crate hex;
extern crate rand;
extern crate log;
extern crate simple_logger;

extern crate wallet;

use wallet::{
    account::AccountAddressType,
    context::{GlobalContext, WalletContext},
    walletlibrary::{WalletLibraryMode, KeyGenConfig},
    mnemonic::Mnemonic,
};
use bitcoin_rpc_client::RpcApi;
use bitcoin::Address;

fn generate_money_for_wallet(context: &mut WalletContext) {
    use std::str::FromStr;

    for address_type in [AccountAddressType::P2PKH, AccountAddressType::P2SHWH, AccountAddressType::P2WKH].iter() {
        let addr = context.wallet_mut()
            .wallet_lib_mut()
            .new_address(address_type.clone())
            .unwrap();
        let change_addr = context.wallet_mut()
            .wallet_lib_mut()
            .new_change_address(address_type.clone())
            .unwrap();
        context.bitcoind_mut()
            .send_to_address(&Address::from_str(&addr).unwrap(), 1.0, None, None, None, None, None, None)
            .unwrap();
        context.bitcoind_mut()
            .send_to_address(&Address::from_str(&change_addr).unwrap(), 1.0, None, None, None, None, None, None)
            .unwrap();
    }

    context.bitcoind_mut().generate(1, None).unwrap();
    context.block_for_sync();
    context.wallet_mut().sync_with_tip().unwrap();
    assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 600_000_000);
}

macro_rules! test {
    ($base:ident) => {
        mod $base {
            use super::{GlobalContext, WalletLibraryMode, $base};
            #[test]
            fn trusted_full_node() {
                let global = GlobalContext::default();
                $base(|mode: WalletLibraryMode| global.default_context(mode).unwrap());
            }
            #[test]
            fn electrumx() {
                let global = GlobalContext::default();
                $base(|mode: WalletLibraryMode| global.electrs_context(mode).unwrap());
            }
        }
    };
}

test!(sanity_check);
test!(base_wallet_functionality);
test!(base_persistent_storage);
test!(extended_persistent_storage);
test!(restore_from_mnemonic);
test!(make_tx_call);
test!(send_coins_call);
test!(lock_coins_flag_success);
test!(lock_coins_flag_fail);

fn sanity_check<F>(make_context: F)
where
    F: Fn(WalletLibraryMode) -> (WalletContext, Mnemonic),
{
    use std::str::FromStr;

    let (mut context, _) = make_context(WalletLibraryMode::Create(KeyGenConfig::default()));
    let _ = context.bitcoind_mut().generate(110, None).unwrap();

    let destination_address = {
        let s = context.wallet_mut()
            .wallet_lib_mut().new_address(AccountAddressType::P2WKH).unwrap();
        Address::from_str(s.as_str()).unwrap()
    };
    let _ = context.bitcoind_mut()
        .send_to_address(&destination_address, 1.0, None, None, None, None, None, None).unwrap();
    let _ = context.bitcoind_mut().generate(1, None).unwrap();
    context.block_for_sync();
    context.wallet_mut().sync_with_tip().unwrap();
    let balance_satoshi = context.wallet_mut().wallet_lib().wallet_balance();
    assert_eq!(balance_satoshi, 100_000_000);
}

fn base_wallet_functionality<F>(make_context: F)
where
    F: Fn(WalletLibraryMode) -> (WalletContext, Mnemonic),
{
    let (mut context, _) = make_context(WalletLibraryMode::Create(KeyGenConfig::default()));
    context.bitcoind_mut().generate(110, None).unwrap();
    generate_money_for_wallet(&mut context);

    // select all available utxos
    // generate destination address
    // check that generated transaction valid and can be send to blockchain
    let ops = context.wallet_mut()
        .wallet_lib()
        .get_utxo_list()
        .iter()
        .map(|utxo| utxo.out_point)
        .collect();
    let dest_addr = context.wallet_mut()
        .wallet_lib_mut()
        .new_address(AccountAddressType::P2WKH)
        .unwrap();
    let tx = context.wallet_mut().make_tx(ops, dest_addr, 150_000_000, true).unwrap();
    context.bitcoind_mut()
        .get_raw_transaction(&tx.txid(), None)
        .unwrap();
}

fn base_persistent_storage<F>(make_context: F)
where
    F: Fn(WalletLibraryMode) -> (WalletContext, Mnemonic),
{
    use std::str::FromStr;

    {
        let (mut context, _) = make_context(WalletLibraryMode::Create(KeyGenConfig::default()));
        context.bitcoind_mut().generate(110, None).unwrap();

        // generate wallet address and send money to it
        let dest_addr = context.wallet_mut()
            .wallet_lib_mut()
            .new_address(AccountAddressType::P2WKH)
            .unwrap();
        context.bitcoind_mut()
            .send_to_address(&Address::from_str(&dest_addr).unwrap(), 1.0, None, None, None, None, None, None)
            .unwrap();
        context.bitcoind_mut().generate(1, None).unwrap();
        context.block_for_sync();
        context.wallet_mut().sync_with_tip().unwrap();
        assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 100_000_000);
    }

    let (mut context, _) = make_context(WalletLibraryMode::Decrypt);

    // balance should not change after restart
    assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 100_000_000);

    // wallet should remain viable after restart, so try to make some ordinary actions
    // and check wallet's state
    let dest_addr = context.wallet_mut()
        .wallet_lib_mut()
        .new_address(AccountAddressType::P2WKH)
        .unwrap();
    context.bitcoind_mut()
        .send_to_address(&Address::from_str(&dest_addr).unwrap(), 1.0, None, None, None, None, None, None)
        .unwrap();
    context.bitcoind_mut().generate(1, None).unwrap();
    context.block_for_sync();
    context.wallet_mut().sync_with_tip().unwrap();
    assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 200_000_000);
}

fn extended_persistent_storage<F>(make_context: F)
where
    F: Fn(WalletLibraryMode) -> (WalletContext, Mnemonic),
{
    {
        let (mut context, _) = make_context(WalletLibraryMode::Create(KeyGenConfig::default()));
        context.bitcoind_mut().generate(110, None).unwrap();
        generate_money_for_wallet(&mut context);
    }

    {
        // recover wallet's state from persistent storage
        // additional scope destroys wallet object(aka wallet restart)
        let (mut context, _) = make_context(WalletLibraryMode::Decrypt);

        // select all available utxos
        // generate destination address
        // spend selected utxos
        let dest_addr = context.wallet_mut()
            .wallet_lib_mut()
            .new_address(AccountAddressType::P2WKH)
            .unwrap();
        let ops = context.wallet_mut()
            .wallet_lib()
            .get_utxo_list()
            .iter()
            .map(|utxo| utxo.out_point)
            .collect();
        let tx = context.wallet_mut().make_tx(ops, dest_addr, 150_000_000, true).unwrap();
        context.bitcoind_mut()
            .get_raw_transaction(&tx.txid(), None)
            .unwrap();
        context.bitcoind_mut().generate(1, None).unwrap();

        context.block_for_sync();
        context.wallet_mut().sync_with_tip().unwrap();

        // wallet send money to itself, so balance decreased only by fee
        assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 600_000_000 - 10_000);
    }

    let (mut context, _) = make_context(WalletLibraryMode::Decrypt);
    // balance should not change after restart
    assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 600_000_000 - 10_000);
}

fn restore_from_mnemonic<F>(make_context: F)
where
    F: Fn(WalletLibraryMode) -> (WalletContext, Mnemonic),
{
    use std::str::FromStr;

    let mnemonic = {
        // initialize wallet with blockchain source and generated money
        // additional scope destroys wallet object(aka wallet restart)
        let (mut context, mnemonic) = make_context(WalletLibraryMode::Create(KeyGenConfig::default()));
        context.bitcoind_mut().generate(110, None).unwrap();
        generate_money_for_wallet(&mut context);
        mnemonic
    };

    // show this string user
    let words_string = mnemonic.to_string();
    // restore mnemonic structure by user's input
    let mnemonic = Mnemonic::from(words_string.as_str()).unwrap();

    // recover wallet's state from mnemonic
    let (mut context, _) = make_context(WalletLibraryMode::RecoverFromMnemonic(mnemonic));

    // balance should not change after restart
    assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 600_000_000);

    // wallet should remain viable after restart, so try to make some ordinary actions
    // and check wallet's state
    let dest_addr = context.wallet_mut()
        .wallet_lib_mut()
        .new_address(AccountAddressType::P2WKH)
        .unwrap();
    context.bitcoind_mut()
        .send_to_address(&Address::from_str(&dest_addr).unwrap(), 1.0, None, None, None, None, None, None)
        .unwrap();
    context.bitcoind_mut().generate(1, None).unwrap();
    context.block_for_sync();
    context.wallet_mut().sync_with_tip().unwrap();
    assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 700_000_000);
}

fn make_tx_call<F>(make_context: F)
where
    F: Fn(WalletLibraryMode) -> (WalletContext, Mnemonic),
{
    // initialize wallet with blockchain source and generated money
    let (mut context, _) = make_context(WalletLibraryMode::Create(KeyGenConfig::default()));
    context.bitcoind_mut().generate(110, None).unwrap();
    generate_money_for_wallet(&mut context);

    // select utxo subset
    // generate destination address
    // spend selected utxo subset
    let ops = context.wallet_mut()
        .wallet_lib()
        .get_utxo_list()
        .iter()
        .take(2)
        .map(|utxo| utxo.out_point)
        .collect();
    let dest_addr = context.wallet_mut()
        .wallet_lib_mut()
        .new_address(AccountAddressType::P2WKH)
        .unwrap();
    let tx = context.wallet_mut().make_tx(ops, dest_addr, 150_000_000, true).unwrap();
    context.bitcoind_mut()
        .get_raw_transaction(&tx.txid(), None)
        .unwrap();
    context.bitcoind_mut().generate(1, None).unwrap();

    context.block_for_sync();
    context.wallet_mut().sync_with_tip().unwrap();

    // wallet send money to itself, so balance decreased only by fee
    assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 600_000_000 - 10_000);

    // we should be able to find utxo with change of previous transaction
    let ok = context.wallet_mut()
        .wallet_lib()
        .get_utxo_list()
        .iter()
        .any(|utxo| utxo.value == 200_000_000 - 150_000_000 - 10_000);
    assert!(ok);
}

fn send_coins_call<F>(make_context: F)
where
    F: Fn(WalletLibraryMode) -> (WalletContext, Mnemonic),
{
    // initialize wallet with blockchain source and generated money
    let (mut context, _) = make_context(WalletLibraryMode::Create(KeyGenConfig::default()));
    context.bitcoind_mut().generate(110, None).unwrap();
    generate_money_for_wallet(&mut context);

    // generate destination address
    // send coins to itself
    // sync with blockchain
    let dest_addr = context.wallet_mut()
        .wallet_lib_mut()
        .new_address(AccountAddressType::P2WKH)
        .unwrap();
    let (tx, _) = context.wallet_mut()
        .send_coins(dest_addr, 150_000_000, false, false, true)
        .unwrap();
    context.bitcoind_mut()
        .get_raw_transaction(&tx.txid(), None)
        .unwrap();
    context.bitcoind_mut().generate(1, None).unwrap();

    context.block_for_sync();
    context.wallet_mut().sync_with_tip().unwrap();

    // wallet send money to itself, so balance decreased only by fee
    assert_eq!(context.wallet_mut().wallet_lib().wallet_balance(), 600_000_000 - 10_000);

    // we should be able to find utxo with change of previous transaction
    let ok = context.wallet_mut()
        .wallet_lib()
        .get_utxo_list()
        .iter()
        .any(|utxo| utxo.value == 200_000_000 - 150_000_000 - 10_000);
    assert!(ok);
}

fn lock_coins_flag_success<F>(make_context: F)
where
    F: Fn(WalletLibraryMode) -> (WalletContext, Mnemonic),
{
    // initialize wallet with blockchain source and generated money
    let (mut context, _) = make_context(WalletLibraryMode::Create(KeyGenConfig::default()));
    context.bitcoind_mut().generate(110, None).unwrap();
    generate_money_for_wallet(&mut context);

    // generate destination address
    // lock all utxos
    // unlock some of them
    // try to lock again
    // should work without errors
    let dest_addr = context.wallet_mut()
        .wallet_lib_mut()
        .new_address(AccountAddressType::P2WKH)
        .unwrap();
    context.wallet_mut()
        .send_coins(dest_addr.clone(), 200_000_000 - 10_000, true, false, false)
        .unwrap();
    context.wallet_mut()
        .send_coins(dest_addr.clone(), 200_000_000 - 10_000, true, false, false)
        .unwrap();
    let (_, lock_id) = context.wallet_mut()
        .send_coins(dest_addr.clone(), 200_000_000 - 10_000, true, false, false)
        .unwrap();
    context.wallet_mut().wallet_lib_mut().unlock_coins(lock_id);

    let (tx, _) = context.wallet_mut()
        .send_coins(dest_addr, 200_000_000 - 10_000, true, false, false)
        .unwrap();
    context.wallet_mut().publish_tx(&tx).unwrap();
}

fn lock_coins_flag_fail<F>(make_context: F)
where
    F: Fn(WalletLibraryMode) -> (WalletContext, Mnemonic),
{
    // initialize wallet with blockchain source and generated money
    let (mut context, _) = make_context(WalletLibraryMode::Create(KeyGenConfig::default()));
    context.bitcoind_mut().generate(110, None).unwrap();
    generate_money_for_wallet(&mut context);

    // generate destination address
    // lock all utxos
    // try to lock again
    // should finish with error
    let dest_addr = context.wallet_mut()
        .wallet_lib_mut()
        .new_address(AccountAddressType::P2WKH)
        .unwrap();
    context.wallet_mut()
        .send_coins(dest_addr.clone(), 200_000_000 - 10_000, true, false, false)
        .unwrap();
    context.wallet_mut()
        .send_coins(dest_addr.clone(), 200_000_000 - 10_000, true, false, false)
        .unwrap();
    context.wallet_mut()
        .send_coins(dest_addr.clone(), 200_000_000 - 10_000, true, false, false)
        .unwrap();

    // should finish with error, no available coins left
    let result = context.wallet_mut().send_coins(dest_addr, 200_000_000 - 10_000, false, false, true);
    assert!(result.is_err());
}

// TODO(evg): tests for lock persistence
// TODO(evg): tests for witness_only flag
