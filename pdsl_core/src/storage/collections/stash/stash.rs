use crate::storage::{
	Key,
	cell::SyncCell,
	chunk::SyncChunk,
};

use parity_codec_derive::{Encode, Decode};

/// A stash collection.
///
/// Provides O(1) random insertion, deletion and access of its elements.
///
/// # Details
///
/// An `O(1)` amortized table that reuses keys.
///
/// ## Guarantees and non-guarantees:
///
/// 1. `Stash` is deterministic and keys do not depend on the inserted values.
///    This means you can update two stashes in tandem and get the same keys
///    back. This could be useful for, e.g., primary/secondary replication.
/// 2. Keys will always be less than the maximum number of items that have ever
///    been present in the `Stash` at any single point in time. In other words,
///    if you never store more than `n` items in a `Stash`, the stash will only
///    assign keys less than `n`. You can take advantage of this guarantee to
///    truncate the key from a `usize` to some smaller type.
/// 3. Except the guarantees noted above, you can assume nothing about key
///    assignment or iteration order. They can change at any time.
#[derive(Debug)]
pub struct Stash<T> {
	/// The latest vacant index.
	next_vacant: SyncCell<u32>,
	/// The number of items stored in the stash.
	///
	/// # Note
	///
	/// We cannot simply use the underlying length of the vector
	/// since it would include vacant slots as well.
	len: SyncCell<u32>,
	/// The maximum length the stash ever had.
	max_len: SyncCell<u32>,
	/// The entries of the stash.
	entries: SyncChunk<Entry<T>>,
}

/// Iterator over the enumerated values of a stash.
pub struct Iter<'a, T> {
	/// The stash that is iterated over.
	stash: &'a Stash<T>,
	/// The index of the current start item of the iteration.
	begin: u32,
	/// The index of the current end item of the iteration.
	end: u32,
	/// The amount of already yielded items.
	///
	/// Required to offer an exact `size_hint` implementation.
	/// Also can be used to exit iteration as early as possible.
	yielded: u32,
}

impl<'a, T> Iter<'a, T> {
	/// Creates a new iterator for the given storage stash.
	pub(crate) fn new(stash: &'a Stash<T>) -> Self {
		Self{
			stash,
			begin: 0,
			end: stash.max_len(),
			yielded: 0,
		}
	}
}

impl<'a, T> Iterator for Iter<'a, T>
where
	T: parity_codec::Codec
{
	type Item = (u32, &'a T);

	fn next(&mut self) -> Option<Self::Item> {
		debug_assert!(self.begin <= self.end);
		if self.yielded == self.stash.len() {
			return None
		}
		while self.begin < self.end {
			let cur = self.begin;
			self.begin += 1;
			if let Some(elem) = self.stash.get(cur) {
				self.yielded += 1;
				return Some((cur, elem))
			}
		}
		None
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining = (self.stash.len() - self.yielded) as usize;
		(remaining, Some(remaining))
	}
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T>
where
	T: parity_codec::Codec
{
	fn next_back(&mut self) -> Option<Self::Item> {
		debug_assert!(self.begin <= self.end);
		if self.yielded == self.stash.len() {
			return None
		}
		while self.begin < self.end {
			self.end -= 1;
			if let Some(elem) = self.stash.get(self.end) {
				self.yielded += 1;
				return Some((self.end, elem))
			}
		}
		None
	}
}

/// An entry within a stash collection.
///
/// This represents either an occupied entry with its associated value
/// or a vacant entry pointing to the next vacant entry.
#[derive(Debug)]
#[derive(Encode, Decode)]
enum Entry<T> {
	/// A vacant entry pointing to the next vacant index.
	Vacant(u32),
	/// An occupied entry containing the value.
	Occupied(T),
}

impl<T> parity_codec::Encode for Stash<T> {
	fn encode_to<W: parity_codec::Output>(&self, dest: &mut W) {
		self.next_vacant.encode_to(dest);
		self.len.encode_to(dest);
		self.max_len.encode_to(dest);
		self.entries.encode_to(dest);
	}
}

impl<T> parity_codec::Decode for Stash<T> {
	fn decode<I: parity_codec::Input>(input: &mut I) -> Option<Self> {
		let next_vacant = SyncCell::decode(input)?;
		let len = SyncCell::decode(input)?;
		let max_len = SyncCell::decode(input)?;
		let entries = SyncChunk::decode(input)?;
		Some(Self{next_vacant, len, max_len, entries})
	}
}

impl<T> Stash<T> {
	/// Creates a new storage vector for the given key.
	///
	/// # Safety
	///
	/// This is an inherently unsafe operation since it does not check
	/// for the storage vector's invariances, such as
	///
	/// - Is the storage region determined by the given key aliasing?
	/// - Is the storage region correctly formatted to be used as storage vec?
	///
	/// Users should not use this routine directly if possible.
	pub unsafe fn new_unchecked(
		next_key: Key,
		len_key: Key,
		max_len_key: Key,
		entries_key: Key
	) -> Self {
		Self{
			next_vacant: SyncCell::new_unchecked(next_key),
			len: SyncCell::new_unchecked(len_key),
			max_len: SyncCell::new_unchecked(max_len_key),
			entries: SyncChunk::new_unchecked(entries_key),
		}
	}

	/// Returns an iterator over the references of all entries of the stash.
	///
	/// # Note
	///
	/// - It is **not** recommended to iterate over all elements of a storage stash.
	/// - Try to avoid this if possible or iterate only over a minimal subset of
	///   all elements using e.g. `Iterator::take(n)`.
	pub fn iter(&self) -> Iter<T> {
		Iter::new(self)
	}

	/// Returns the unterlying key to the cells.
	///
	/// # Note
	///
	/// This is a low-level utility getter and should
	/// normally not be required by users.
	pub fn entries_key(&self) -> Key {
		self.entries.cells_key()
	}

	/// Returns the number of elements stored in the stash.
	pub fn len(&self) -> u32 {
		*self.len.get().unwrap_or(&0)
	}

	/// Returns the maximum number of element stored in the
	/// stash at the same time.
	pub fn max_len(&self) -> u32 {
		*self.max_len.get().unwrap_or(&0)
	}

	/// Returns `true` if the stash contains no elements.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Returns the next vacant index.
	fn next_vacant(&self) -> u32 {
		*self.next_vacant.get().unwrap_or(&0)
	}

}

impl<T> Stash<T>
where
	T: parity_codec::Codec,
{
	/// Returns the element stored at index `n` if any.
	pub fn get(&self, n: u32) -> Option<&T> {
		self
			.entries
			.get(n)
			.and_then(|entry| match entry {
				Entry::Occupied(val) => Some(val),
				Entry::Vacant(_) => None,
			})
	}

	/// Put the element into the stash at the next vacant position.
	///
	/// Returns the stash index that the element was put into.
	pub fn put(&mut self, val: T) -> u32 {
		let current_vacant = *self
			.next_vacant
			.get()
			.unwrap_or(&0);
		debug_assert!(current_vacant <= self.len());
		if current_vacant == self.len() {
			self.entries.set(current_vacant, Entry::Occupied(val));
			self.next_vacant.set(current_vacant + 1);
			self.max_len.set(self.max_len() + 1);
		} else {
			let next_vacant = match
				self
					.entries
					.replace(current_vacant, Entry::Occupied(val))
					.expect(
						"[pdsl_core::Stash::put] Error: \
						expected a vacant entry here, but no entry was found"
					)
				{
					Entry::Vacant(next_vacant) => next_vacant,
					Entry::Occupied(_) => unreachable!(
						"[pdsl_core::Stash::put] Error: \
						a next_vacant index can never point to an occupied entry"
					)
				};
			self.next_vacant.set(next_vacant);
		}
		self.len.set(self.len() + 1);
		current_vacant
	}

	/// Takes the element stored at index `n`-th if any.
	pub fn take(&mut self, n: u32) -> Option<T> {
		match self.entries.get(n) {
			| None
			| Some(Entry::Vacant(_)) => None,
			| Some(Entry::Occupied(_)) => {
				match self.entries.replace(n, Entry::Vacant(self.next_vacant())).expect(
					"[pdsl_core::Stash::take] Error: \
					 we already asserted that the entry at `n` exists"
				) {
					Entry::Occupied(val) => {
						self.next_vacant.set(n);
						debug_assert!(self.len() >= 1);
						self.len.set(self.len() - 1);
						Some(val)
					},
					Entry::Vacant(_) => unreachable!(
						"[pdsl_core::Stash::take] Error: \
						 we already asserted that the entry is occupied"
					)
				}
			}
		}
	}
}
