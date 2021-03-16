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

pub mod ext;
pub mod test_api;

mod exec_context;
mod hashing;
mod storage;
mod types;

pub use test_api::EmittedEvent;
pub use types::AccountError;

use types::Key;

use derive_more::From;

/// Errors which can happen when interacting with this crate.
#[derive(Debug, From, PartialEq, Eq)]
pub enum Error {
    Account(AccountError),
    #[from(ignore)]
    UninitializedBlocks,
    #[from(ignore)]
    UninitializedExecutionContext,
    #[from(ignore)]
    UnregisteredChainExtension,
}

pub trait OnInstance {
    fn on_instance<F, R>(f: F) -> R
    where
        F: FnOnce(&mut Self) -> R;
}
