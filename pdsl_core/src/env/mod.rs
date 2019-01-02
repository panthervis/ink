//! Contract environments.
//!
//! A contract is able to operate on different environments.
//!
//! Currently the SRML environment operating directly on the
//! substrate runtime module library (SRML) and the test
//! environment for testing and inspecting contracts are
//! provided.
//!
//! By default the SRML environment is used.
//! To enable the test environment the `test-env` crate feature
//! has to be enabled.

#[cfg(not(feature = "test-env"))]
mod srml_env;

#[cfg(feature = "test-env")]
mod test_env;

use crate::storage::Key;

/// The evironment API usable by SRML contracts.
pub trait Env {
	/// Returns the chain address of the caller.
	fn caller() -> Vec<u8>;
	/// Stores the given value under the given key.
	fn store(key: Key, value: &[u8]);
	/// Clears the value stored under the given key.
	fn clear(key: Key);
	/// Loads data stored under the given key.
	///
	/// # Safety
	///
	/// Is unsafe since there is no check for key integrity.
	/// This operation can be compared to a pointer deref in Rust
	/// which itself is also considered unsafe.
	unsafe fn load(key: Key) -> Option<Vec<u8>>;
	/// Loads input data for contract execution.
	fn input() -> Vec<u8>;
	/// Returns from the contract execution with the given value.
	fn return_(value: &[u8]) -> !;
}

#[cfg(not(feature = "test-env"))]
pub use self::srml_env::SrmlEnv;

#[cfg(feature = "test-env")]
pub use self::test_env::TestEnv;

/// The environment implementation that is currently being used.
///
/// This may be either
/// - `DefaultEnv` for real contract storage
///   manipulation that may happen on-chain.
/// - `TestEnv` for emulating a contract environment
///   that can be inspected by the user and used
///   for testing contracts off-chain.
#[cfg(not(feature = "test-env"))]
pub type ContractEnv = self::srml_env::SrmlEnv;

/// The environment implementation that is currently being used.
#[cfg(feature = "test-env")]
pub type ContractEnv = self::test_env::TestEnv;
