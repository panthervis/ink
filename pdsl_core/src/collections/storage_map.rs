use crate::storage::{Key, Synced, SyncedChunk};
use crate::hash::{self, HashAsKeccak256};

use std::borrow::Borrow;

/// Mapping stored in the contract storage.
///
/// # Note
///
/// This performs a quadratic probing on the next 2^32 slots
/// following its initial key. So it can store up to 2^32 elements in total.
///
/// Instead of storing element values (`V`) directly, it stores
/// storage map entries of `(K, V)` instead. This allows to represent
/// the storage that is associated to the storage map to be in three
/// different states.
///
/// 1. Occupied slot with key and value.
/// 2. Removed slot that was occupied before.
/// 3. Empty slot when there never was an insertion for this storage slot.
///
/// This distinction is important for the quadratic map probing.
#[derive(Debug)]
pub struct StorageMap<K, V> {
	/// The storage key to the length of this storage map.
	len: Synced<u32>,
	/// The first half of the entry buffer is equal to the key,
	/// the second half will be replaced with the respective
	/// hash of any given key upon usage.
	///
	/// Afterwards this value is hashed again and used as key
	/// into the contract storage.
	entries: SyncedChunk<Entry<K, V>>,
}

/// An entry of a storage map.
///
/// This can either store the entries key and value
/// or represent an entry that was removed after it
/// has been occupied with key and value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(parity_codec_derive::Encode, parity_codec_derive::Decode)]
pub enum Entry<K, V> {
	/// An occupied slot with a key and a value.
	Occupied(OccupiedEntry<K, V>),
	/// A removed slot that was occupied before.
	Removed,
}

/// An occupied entry of a storage map.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(parity_codec_derive::Encode, parity_codec_derive::Decode)]
pub struct OccupiedEntry<K, V> {
	/// The entry's key.
	key: K,
	/// The entry's value.
	val: V,
}

impl<K, V> From<Key> for StorageMap<K, V>
where
	K: parity_codec::Codec,
	V: parity_codec::Codec,
{
	fn from(key: Key) -> Self {
		StorageMap{
			len: Synced::from(key),
			entries: SyncedChunk::from(
				Key::with_offset(&key, 1)
			),
		}
	}
}

impl<K, V> StorageMap<K, V> {
	/// Returns the number of key-value pairs in the map.
	pub fn len(&self) -> u32 {
		*self.len.get()
	}

	/// Returns `true` if the map contains no elements.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}

/// Converts the given bytes into a `u32` value.
///
/// The first byte in the array will be the most significant byte.
fn bytes_to_u32(bytes: [u8; 4]) -> u32 {
	let mut res = 0;
	res |= (bytes[0] as u32) << 24;
	res |= (bytes[1] as u32) << 16;
	res |= (bytes[2] as u32) <<  8;
	res |= (bytes[3] as u32) <<  0;
	res
}

/// Converts the given slice into an array with fixed size of 4.
///
/// Returns `None` if the slice's length is not 4.
fn slice_as_array4<T>(bytes: &[T]) -> Option<[T; 4]>
where
	T: Default + Copy
{
	if bytes.len() != 4 {
		return None
	}
	let mut array = [T::default(); 4];
	for i in 0..4 {
		array[i] = bytes[i];
	}
	Some(array)
}

impl<K, V> StorageMap<K, V>
where
	K: parity_codec::Codec + HashAsKeccak256 + Eq,
	V: parity_codec::Codec,
{
	/// Probes for a free or usable slot.
	///
	/// # Note
	///
	/// - Uses quadratic probing.
	/// - Returns `(true, _)` if there was a key-match of an already
	///   occupied slot, returns `(false, _)` if the found slot is empty.
	/// - Returns `(_, n)` if `n` is the found probed index.
	fn probe<Q>(&self, key: &Q, inserting: bool) -> (bool, u32)
	where
		K: Borrow<Q>,
		Q: HashAsKeccak256 + Eq
	{
		// Convert the first 4 bytes in the keccak256 hash
		// of the key into a big-endian unsigned integer.
		let probe_start = bytes_to_u32(
			slice_as_array4(
				&(hash::keccak256(key.borrow())[0..4])
			).expect(
				"[pdsl_core::StorageMap::insert] Error \
				 couldn't convert to probe_start byte array"
			)
		);
		// This is the offset for the quadratic probing.
		let mut probe_hops = 0;
		let mut probe_offset = 0;
		'outer: loop {
			let probe_index = probe_start.wrapping_add(probe_offset);
			match self.entries.get(probe_index) {
				Some(Entry::Occupied(entry)) => {
					if key == entry.key.borrow() {
						return (true, probe_index)
					}
					// Need to jump using quadratic probing.
					probe_hops += 1;
					probe_offset = probe_hops * probe_hops;
					continue 'outer
				}
				Some(Entry::Removed) | None => {
					// We can insert into this slot.
					if inserting {
						return (false, probe_index)
					}
					continue 'outer
				}
			}
		}
	}

	/// Probes for a free or usable slot while inserting.
	///
	/// # Note
	///
	/// For more information refer to the `fn probe` documentation.
	fn probe_inserting<Q>(&self, key: &Q) -> (bool, u32)
	where
		K: Borrow<Q>,
		Q: HashAsKeccak256 + Eq
	{
		self.probe(key, true)
	}

	/// Probes for a free or usable slot while inspecting.
	///
	/// # Note
	///
	/// For more information refer to the `fn probe` documentation.
	fn probe_inspecting<Q>(&self, key: &Q) -> u32
	where
		K: Borrow<Q>,
		Q: HashAsKeccak256 + Eq
	{
		self.probe(key, false).1
	}

	/// Inserts a key-value pair into the map.
	///
	/// If the map did not have this key present, `None` is returned.
	///
	/// If the map did have this key present, the value is updated,
	/// and the old value is returned.
	/// The key is not updated, though;
	/// this matters for types that can be == without being identical.
	/// See the module-level documentation for more.
	pub fn insert(&mut self, key: K, val: V) -> Option<V> {
		match self.probe_inserting(&key) {
			(true, probe_index) => {
				// Keys match, values might not.
				// So we have to overwrite this entry with the new value.
				let old = self.entries.remove(probe_index);
				self.entries.insert(
					probe_index, Entry::Occupied(OccupiedEntry{key, val})
				);
				return match old.unwrap() {
					Entry::Occupied(OccupiedEntry{val, ..}) => Some(val),
					Entry::Removed => None,
				}
			}
			(false, probe_index) => {
				// We can insert into this slot.
				self.entries.insert(
					probe_index,
					Entry::Occupied(OccupiedEntry{key, val})
				);
				return None
			}
		}
	}

	/// Removes a key from the map,
	/// returning the value at the key if the key was previously in the map.
	///
	/// # Note
	///
	/// The key may be any borrowed form of the map's key type,
	/// but Hash and Eq on the borrowed form must match those for the key type.
	pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
	where
		K: Borrow<Q>,
		Q: HashAsKeccak256 + Eq
	{
		let probe_index = self.probe_inspecting(key);
		match self.entries.remove(probe_index) {
			Some(Entry::Removed) | None => None,
			Some(Entry::Occupied(OccupiedEntry{val, ..})) => Some(val),
		}
	}

	/// Returns the value corresponding to the key.
	///
	/// The key may be any borrowed form of the map's key type,
	/// but Hash and Eq on the borrowed form must match those for the key type.
	pub fn get<Q>(&self, key: &Q) -> Option<&V>
	where
		K: Borrow<Q>,
		Q: HashAsKeccak256 + Eq
	{
		match self.entry(key) {
			Some(Entry::Removed) | None => None,
			Some(Entry::Occupied(OccupiedEntry{val, ..})) => Some(val),
		}
	}

	/// Returns the entry corresponding to the key.
	///
	/// The key may be any borrowed form of the map's key type,
	/// but Hash and Eq on the borrowed form must match those for the key type.
	pub fn entry<Q>(&self, key: &Q) -> Option<&Entry<K, V>>
	where
		K: Borrow<Q>,
		Q: HashAsKeccak256 + Eq
	{
		self.entries.get(self.probe_inspecting(key))
	}
}