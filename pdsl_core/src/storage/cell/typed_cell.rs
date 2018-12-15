use crate::{
	storage::{
		Key,
		NonCloneMarker,
		cell::RawCell,
	},
};

/// A typed cell.
///
/// Provides interpreted access to the associated contract storage slot.
///
/// # Guarantees
///
/// - `Owned`
/// - `Typed`
///
/// Read more about kinds of guarantees and their effect [here](../index.html#guarantees).
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct TypedCell<T> {
	/// The associated raw cell.
	cell: RawCell,
	/// Marker that prevents this type from being `Copy` or `Clone` by accident.
	non_clone: NonCloneMarker<T>,
}

impl<T> TypedCell<T> {
	/// Creates a new typed cell for the given key.
	///
	/// # Note
	///
	/// This is unsafe since it does not check if the associated
	/// contract storage does not alias with other accesses.
	pub unsafe fn new_unchecked(key: Key) -> Self {
		Self{
			cell: RawCell::new_unchecked(key),
			non_clone: NonCloneMarker::default()
		}
	}
}

impl<T> TypedCell<T>
where
	T: parity_codec::Decode
{
	/// Loads the typed entity if any.
	pub fn load(&self) -> Option<T> {
		self.cell.load().and_then(|bytes| T::decode(&mut &bytes[..]))
	}
}

impl<T> TypedCell<T>
where
	T: parity_codec::Encode
{
	/// Stores the given entity.
	pub fn store(&mut self, val: &T) {
		self.cell.store(&T::encode(&val))
	}

	/// Removes the entity in the associated constract storage slot.
	pub fn clear(&mut self) {
		self.cell.clear()
	}
}

#[cfg(all(test, feature = "test-env"))]
mod tests {
	use super::*;

	use crate::env::TestEnv;

	#[test]
	fn simple() {
		let mut cell: TypedCell<i32> = unsafe {
			TypedCell::new_unchecked(Key([0x42; 32]))
		};
		assert_eq!(cell.load(), None);
		cell.store(&5);
		assert_eq!(cell.load(), Some(5));
		cell.clear();
		assert_eq!(cell.load(), None);
	}

	#[test]
	fn count_reads() {
		let cell: TypedCell<i32> = unsafe {
			TypedCell::new_unchecked(Key([0x42; 32]))
		};
		assert_eq!(TestEnv::total_reads(), 0);
		cell.load();
		assert_eq!(TestEnv::total_reads(), 1);
		cell.load();
		cell.load();
		assert_eq!(TestEnv::total_reads(), 3);
	}

	#[test]
	fn count_writes() {
		let mut cell: TypedCell<i32> = unsafe {
			TypedCell::new_unchecked(Key([0x42; 32]))
		};
		assert_eq!(TestEnv::total_writes(), 0);
		cell.store(&1);
		assert_eq!(TestEnv::total_writes(), 1);
		cell.store(&2);
		cell.store(&3);
		assert_eq!(TestEnv::total_writes(), 3);
	}
}
