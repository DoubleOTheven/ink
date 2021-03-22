// Copyright 2018-2021 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Provides the same interface as Substrate's FRAME `contract` module.
//!
//! See [the documentation for the `contract` module](https://docs.rs/crate/pallet-contracts)
//! for more information.

use crate::{
    exec_context::ExecContext,
    storage::Storage,
    test_api::{
        self,
        EmittedEvent,
        RecInstance,
    },
    types::{
        AccountId,
        Balance,
        Key,
    },
    OnInstance,
};

use core::cell::RefCell;
use std::collections::HashMap;

type Result = core::result::Result<(), Error>;

pub struct EnvInstance {
    /// The environment storage.
    pub storage: Storage<Key, Vec<u8>>,
    /// Holds the balance of each account.
    pub balances: HashMap<AccountId, Balance>,
    /// The current execution context.
    pub exec_context: ExecContext,
}

impl OnInstance for EnvInstance {
    fn on_instance<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        thread_local!(
            static ENV_INSTANCE: RefCell<EnvInstance> = RefCell::new(EnvInstance {
                storage: Storage::new(),
                balances: HashMap::new(),
                exec_context: ExecContext::default(),
            });
        );
        ENV_INSTANCE.with(|instance| f(&mut instance.borrow_mut()))
    }
}

macro_rules! define_error_codes {
    (
        $(
            $( #[$attr:meta] )*
            $name:ident = $discr:literal,
        )*
    ) => {
        /// Every error that can be returned to a contract when it calls any of the host functions.
        #[cfg_attr(test, derive(PartialEq, Eq))]
        #[derive(Debug)]
        #[repr(u32)]
        pub enum Error {
            $(
                $( #[$attr] )*
                $name = $discr,
            )*
            /// Returns if an unknown error was received from the host module.
            UnknownError,
        }

        impl From<ReturnCode> for Result {
            #[inline]
            fn from(return_code: ReturnCode) -> Self {
                match return_code.0 {
                    0 => Ok(()),
                    $(
                        $discr => Err(Error::$name),
                    )*
                    _ => Err(Error::UnknownError),
                }
            }
        }
    };
}
define_error_codes! {
    /// The called function trapped and has its state changes reverted.
    /// In this case no output buffer is returned.
    /// Can only be returned from `seal_call` and `seal_instantiate`.
    CalleeTrapped = 1,
    /// The called function ran to completion but decided to revert its state.
    /// An output buffer is returned when one was supplied.
    /// Can only be returned from `seal_call` and `seal_instantiate`.
    CalleeReverted = 2,
    /// The passed key does not exist in storage.
    KeyNotFound = 3,
    /// Transfer failed because it would have brought the sender's total balance
    /// below the subsistence threshold.
    BelowSubsistenceThreshold = 4,
    /// Transfer failed for other not further specified reason. Most probably
    /// reserved or locked balance of the sender that was preventing the transfer.
    TransferFailed = 5,
    /// The newly created contract is below the subsistence threshold after executing
    /// its constructor so no usable contract instance will be created.
    NewContractNotFunded = 6,
    /// No code could be found at the supplied code hash.
    CodeNotFound = 7,
    /// The account that was called is either no contract (e.g. user account) or is a tombstone.
    NotCallable = 8,
}

/// The raw return code returned by the host side.
#[repr(transparent)]
pub struct ReturnCode(u32);

impl ReturnCode {
    /// Returns the raw underlying `u32` representation.
    pub fn into_u32(self) -> u32 {
        self.0
    }
}

fn extract_from_slice(output: &mut &mut [u8], new_len: usize) {
    debug_assert!(new_len <= output.len());
    let tmp = core::mem::take(output);
    *output = &mut tmp[..new_len];
}

/// Transfers value from the contract to the destination account.
pub fn transfer(account_id: &[u8], mut value: &[u8]) -> Result {
    // Note that a transfer of `0` is allowed here
    let increment =
        <u128 as scale::Decode>::decode(&mut value).map_err(|_| Error::TransferFailed)?;

    let dest = account_id.to_vec();
    // Note that the destination account does not have to exist
    let dest_old_balance = test_api::get_balance(dest.clone()).unwrap_or_default();

    let contract = test_api::get_callee();
    let contract_old_balance =
        test_api::get_balance(contract.clone()).map_err(|_| Error::TransferFailed)?;

    test_api::set_balance(contract, contract_old_balance - increment);
    test_api::set_balance(dest, dest_old_balance + increment);
    Ok(())
}

/// Deposits an event identified by the supplied topics and data.
pub fn deposit_event(topics: &[u8], data: &[u8]) {
    // The first byte contains the number of topics in the slice
    let topics_count: scale::Compact<u32> = scale::Decode::decode(&mut &topics[0..1])
        .expect("decoding number of topics failed");
    let topics_count = topics_count.0 as usize;

    let topics_vec = if topics_count > 0 {
        // The rest of the slice contains the topics
        let topics = &topics[1..];
        let bytes_per_topic = topics.len() / topics_count;
        let topics_vec: Vec<Vec<u8>> = topics
            .chunks(bytes_per_topic)
            .map(|chunk| chunk.to_vec())
            .collect();
        assert_eq!(topics_count, topics_vec.len());
        topics_vec
    } else {
        Vec::new()
    };

    <RecInstance as OnInstance>::on_instance(|instance| {
        instance.record_event(EmittedEvent {
            topics: topics_vec,
            data: data.to_vec(),
        });
    });
}

/// Writes the encoded value into the contract storage at the given key.
pub fn set_storage(key: &[u8], encoded_value: &[u8]) {
    <RecInstance as OnInstance>::on_instance(|instance| {
        let account_id: AccountId = test_api::get_callee().into();
        instance.inc_writes(account_id.clone());
        instance.inc_cells_per_account(account_id);
    });

    <EnvInstance as OnInstance>::on_instance(|instance| {
        // We ignore if storage is already set for this key
        let _ = instance.storage.insert(key.into(), encoded_value.to_vec());
    })
}

/// Removes the value from storage entries at the given key.
pub fn clear_storage(key: &[u8]) {
    <RecInstance as OnInstance>::on_instance(|instance| {
        let account_id: AccountId = test_api::get_callee().into();
        instance.inc_writes(account_id.clone());
        instance.dec_cells_per_account(account_id);
    });

    <EnvInstance as OnInstance>::on_instance(|instance| {
        instance.storage.remove(&key.into());
    })
}

/// Returns the decoded storage at the key if any.
pub fn get_storage(key: &[u8], output: &mut &mut [u8]) -> Result {
    <RecInstance as OnInstance>::on_instance(|instance| {
        let account_id = test_api::get_callee().into();
        instance.inc_reads(account_id);
    });

    <EnvInstance as OnInstance>::on_instance(|instance| {
        match instance.storage.get(&key.into()) {
            Some(val) => {
                output[0..val.len()].copy_from_slice(val);
                Ok(())
            }
            None => Err(Error::KeyNotFound),
        }
    })
}

/// Remove the calling account and transfer remaining balance.
///
/// This function never returns. Either the termination was successful and the
/// execution of the destroyed contract is halted. Or it failed during the
/// termination which is considered fatal.
pub fn terminate(beneficiary: &[u8]) -> ! {
    // Send the remaining balance to the beneficiary
    let contract = test_api::get_callee();
    let all = test_api::get_balance(contract).expect("could not get balance");
    let value = &scale::Encode::encode(&all)[..];
    transfer(beneficiary, value).expect("transfer did not work");

    // What is currently missing is to set a tombstone with a code hash here
    // and remove the contract storage subsequently.

    // Encode the result of the termination and panic with it.
    // This enables testing for the proper result and makes sure this
    // method returns `Never`.
    let res = (all, beneficiary.to_vec());
    panic!("{:?}", scale::Encode::encode(&res));
}

/// Returns the address of the caller.
pub fn caller(output: &mut &mut [u8]) {
    <EnvInstance as OnInstance>::on_instance(|instance| {
        let caller: Vec<u8> = instance.exec_context.caller.clone().into();
        output[..caller.len()].copy_from_slice(&caller[..]);
        extract_from_slice(output, caller.len());
    });
}

/// Returns the balance of the executed contract.
pub fn balance(output: &mut &mut [u8]) {
    <EnvInstance as OnInstance>::on_instance(|instance| {
        let contract = &instance.exec_context.callee;
        let balance: Vec<u8> = scale::Encode::encode(
            instance
                .balances
                .get(contract)
                .expect("currently executing contract must exist"),
        );
        output[..balance.len()].copy_from_slice(&balance[..]);
        extract_from_slice(output, balance.len());
    });
}

/// Returns the transferred value for the called contract.
pub fn value_transferred(output: &mut &mut [u8]) {
    <EnvInstance as OnInstance>::on_instance(|instance| {
        let value_transferred: Vec<u8> =
            scale::Encode::encode(&instance.exec_context.value_transferred);
        output[..value_transferred.len()].copy_from_slice(&value_transferred[..]);
        extract_from_slice(output, value_transferred.len());
    });
}

/// Returns the address of the executed contract.
pub fn address(output: &mut &mut [u8]) {
    <EnvInstance as OnInstance>::on_instance(|instance| {
        let callee: Vec<u8> = instance.exec_context.callee.clone().into();
        output[..callee.len()].copy_from_slice(&callee[..]);
        extract_from_slice(output, callee.len());
    });
}

/// Restores a tombstone to the original smart contract.
///
/// # Params
///
/// - `account_id`: Encoded bytes of the `AccountId` of the to-be-restored contract.
/// - `code_hash`: Encoded code hash of the to-be-restored contract.
/// - `rent_allowance`: The encoded rent allowance of the restored contract
///                     upon successful restoration.
/// - `filtered_keys`: Storage keys that will be ignored for the tombstone hash
///                    match calculation that decide whether the original contract
///                    storage and the storage of the restorer contract is equal.
pub fn restore_to(
    _account_id: &[u8],
    _code_hash: &[u8],
    _rent_allowance: &[u8],
    filtered_keys: &[Vec<u8>],
) {
    let _filtered_keys: Vec<crate::Key> = filtered_keys.iter().map(Into::into).collect();
    unimplemented!("off-chain environment does not yet support `restore_to`");
}

/// Prints the given contents to the console log.
pub fn println(content: &str) {
    <RecInstance as OnInstance>::on_instance(|instance| {
        instance.record_println(String::from(content))
    });
    println!("{}", content);
}

/// Conduct the BLAKE-2 256-bit hash and place the result into `output`.
pub fn hash_blake2_256(input: &[u8], output: &mut [u8; 32]) {
    super::hashing::blake2b_256(input, output);
}

/// Conduct the BLAKE-2 128-bit hash and place the result into `output`.
pub fn hash_blake2_128(input: &[u8], output: &mut [u8; 16]) {
    super::hashing::blake2b_128(input, output);
}

/// Conduct the SHA-2 256-bit hash and place the result into `output`.
pub fn hash_sha2_256(input: &[u8], output: &mut [u8; 32]) {
    super::hashing::sha2_256(input, output);
}

/// Conduct the KECCAK 256-bit hash and place the result into `output`.
pub fn hash_keccak_256(input: &[u8], output: &mut [u8; 32]) {
    super::hashing::keccak_256(input, output);
}

pub fn now(_output: &mut &mut [u8]) {
    unimplemented!("off-chain environment does not yet support `now`");
}

pub fn block_number(_output: &mut &mut [u8]) {
    unimplemented!("off-chain environment does not yet support `block_number`");
}

pub fn gas_left(_output: &mut &mut [u8]) {
    unimplemented!("off-chain environment does not yet support `gas_left`");
}

pub fn rent_allowance(_output: &mut &mut [u8]) {
    unimplemented!("off-chain environment does not yet support `rent_allowance`");
}

pub fn minimum_balance(_output: &mut &mut [u8]) {
    unimplemented!("off-chain environment does not yet support `minimum_balance`");
}

pub fn tombstone_deposit(_output: &mut &mut [u8]) {
    unimplemented!("off-chain environment does not yet support `tombstone_deposit`");
}

pub fn instantiate(
    _code_hash: &[u8],
    _gas_limit: u64,
    _endowment: &[u8],
    _input: &[u8],
    _out_address: &mut &mut [u8],
    _out_return_value: &mut &mut [u8],
    _salt: &[u8],
) -> Result {
    unimplemented!("off-chain environment does not yet support `instantiate`");
}

pub fn call(
    _callee: &[u8],
    _gas_limit: u64,
    _value: &[u8],
    _input: &[u8],
    _output: &mut &mut [u8],
) -> Result {
    unimplemented!("off-chain environment does not yet support `call`");
}

pub fn weight_to_fee(_gas: u64, _output: &mut &mut [u8]) {
    unimplemented!("off-chain environment does not yet support `weight_to_fee`");
}

pub fn set_rent_allowance(_value: &[u8]) {
    unimplemented!("off-chain environment does not yet support `set_rent_allowance`");
}

pub fn random(_subject: &[u8], _output: &mut &mut [u8]) {
    unimplemented!("off-chain environment does not yet support `random`");
}

pub fn call_chain_extension(
    _func_id: u32,
    _input: &[u8],
    _output: &mut &mut [u8],
) -> u32 {
    unimplemented!("off-chain environment does not yet support `call_chain_extension`");
}