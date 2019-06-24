// Copyright 2018-2019 Parity Technologies (UK) Ltd.
// This file is part of ink!.
//
// ink! is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// ink! is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with ink!.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(all(test, feature = "test-env")), no_std)]

use parity_codec::{
    Decode,
    Encode,
};
use ink_core::{
    env::{
        self,
        ContractEnv,
        DefaultSrmlTypes,
        EnvTypes,
        Env as _,
    },
    storage::{
        self,
        alloc::{
            AllocateUsing,
			Allocate,
            BumpAlloc,
            Initialize,
        },
        Flush,
        Key,
    },
};

type AccountId = <ContractEnv<DefaultSrmlTypes> as EnvTypes>::AccountId;
type Balance = <ContractEnv<DefaultSrmlTypes> as EnvTypes>::Balance;

/// The storage data that is hold by the ERC-20 token.
#[derive(Debug, Encode, Decode)]
pub struct Erc20Token {
    /// All peeps done by all users.
    balances: storage::HashMap<AccountId, Balance>,
    /// Balances that are spendable by non-owners.
    ///
    /// # Note
    ///
    /// Mapping: (from, to) -> allowed
    allowances: storage::HashMap<(AccountId, AccountId), Balance>,
    /// The total supply.
    total_supply: storage::Value<Balance>,
}

impl Erc20Token {
    /// Returns the total number of tokens in existence.
    pub fn total_supply(&self) -> Balance {
        *self.total_supply
    }

    /// Returns the balance of the given address.
    pub fn balance_of(&self, owner: AccountId) -> Balance {
        *self.balances.get(&owner).unwrap_or(&0)
    }

    /// Returns the amount of tokens that an owner allowed to a spender.
    pub fn allowance(&self, owner: AccountId, spender: AccountId) -> Balance {
        *self.allowances.get(&(owner, spender)).unwrap_or(&0)
    }

    /// Transfers token from the sender to the `to` address.
    pub fn transfer(&mut self, to: AccountId, value: Balance) -> bool {
        self.transfer_impl(ContractEnv::<DefaultSrmlTypes>::caller(), to, value);
        true
    }

    /// Approve the passed address to spend the specified amount of tokens
    /// on the behalf of the message's sender.
    ///
    /// # Note
    ///
    /// Beware that changing an allowance with this method afterwards brings
    /// the risk that someone may use both, the old and the new allowance,
    /// by unfortunate transaction ordering.
    /// One possible solution to mitigate this race condition is to first reduce
    /// the spender's allowance to 0 and set the desired value afterwards:
    /// https://github.com/ethereum/EIPs/issues/20#issuecomment-263524729
    pub fn approve(&mut self, spender: AccountId, value: Balance) -> bool {
        let owner = ContractEnv::<DefaultSrmlTypes>::caller();
        self.allowances.insert((owner, spender), value);
        // emit event (not ready yet)
        true
    }

    /// Transfer tokens from one address to another.
    ///
    /// Note that while this function emits an approval event,
    /// this is not required as per the specification,
    /// and other compliant implementations may not emit the event.
    pub fn transfer_from(&mut self, from: AccountId, to: AccountId, value: Balance) -> bool {
        self.allowances[&(from, to)] -= value;
        self.transfer_impl(from, to, value);
        // emit approval(from, to, value) (not yet ready)
        true
    }

    /// Transfers token from a specified address to another address.
    fn transfer_impl(&mut self, from: AccountId, to: AccountId, value: Balance) {
        self.balances[&from] -= value;
        self.balances[&to] += value;
        // emit transfer(from, to, value) (not ready yet)
    }

    // fn mint_for(&mut self, receiver: AccountId, value: Balance) {
    // 	self.balances[&receiver] += value;
    // 	self.total_supply += value;
    // 	// emit transfer(0, receiver, value) (not ready yet)
    // }
}

// BELOW THIS EVERYTHING WILL EVENTUALLY BE GENERATED BY THE eDSL

impl AllocateUsing for Erc20Token {
    unsafe fn allocate_using<A>(alloc: &mut A) -> Self
    where
        A: Allocate,
    {
        Self {
            balances: storage::HashMap::allocate_using(alloc),
            allowances: storage::HashMap::allocate_using(alloc),
            total_supply: storage::Value::allocate_using(alloc),
        }
    }
}

impl Initialize for Erc20Token {
    type Args = ();

    fn initialize(&mut self, _params: Self::Args) {
        // self.mint_for(alice_address(), 10_000);
        // self.mint_for(bob_address(), 500);
    }
}

impl Flush for Erc20Token {
    fn flush(&mut self) {
        self.balances.flush();
        self.allowances.flush();
        self.total_supply.flush();
    }
}

/// Erc20Token API.
#[derive(Encode, Decode)]
enum Action {
    TotalSupply, // -> Balance
    BalanceOf {
        owner: AccountId,
    }, // -> Balance
    Allowance {
        owner: AccountId,
        spender: AccountId,
    }, // -> Balance
    Transfer {
        to: AccountId,
        value: Balance,
    }, // -> bool
    Approve {
        spender: AccountId,
        value: Balance,
    }, // -> bool
    TransferFrom {
        from: AccountId,
        to: AccountId,
        value: Balance,
    }, // -> bool
}

fn ret<T>(val: T) -> !
where
    T: parity_codec::Encode,
{
    unsafe { env::r#return::<T, ContractEnv<DefaultSrmlTypes>>(val) }
}

fn instantiate() -> Erc20Token {
    unsafe {
        let mut alloc = BumpAlloc::from_raw_parts(Key([0x0; 32]));
        Erc20Token::allocate_using(&mut alloc)
    }
}

#[no_mangle]
pub extern "C" fn deploy() {
    instantiate().initialize_into(()).flush()
}

fn decode_params() -> Action {
    let input = ContractEnv::<DefaultSrmlTypes>::input();
    Action::decode(&mut &input[..]).unwrap()
}

#[no_mangle]
pub extern "C" fn call() {
    let mut erc20token = instantiate();
    match decode_params() {
        Action::TotalSupply => {
            let ret_val = erc20token.total_supply();
            erc20token.flush();
            ret(ret_val);
        }
        Action::BalanceOf { owner } => {
            let ret_val = erc20token.balance_of(owner);
            erc20token.flush();
            ret(ret_val);
        }
        Action::Allowance { owner, spender } => {
            let ret_val = erc20token.allowance(owner, spender);
            erc20token.flush();
            ret(ret_val);
        }
        Action::Transfer { to, value } => {
            let ret_val = erc20token.transfer(to, value);
            erc20token.flush();
            ret(ret_val);
        }
        Action::Approve { spender, value } => {
            let ret_val = erc20token.approve(spender, value);
            erc20token.flush();
            ret(ret_val);
        }
        Action::TransferFrom { from, to, value } => {
            let ret_val = erc20token.transfer_from(from, to, value);
            erc20token.flush();
            ret(ret_val);
        }
    }
}
