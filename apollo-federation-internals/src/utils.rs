use indexmap::IndexMap;
use hashlink::{LinkedHashMap, LinkedHashSet};
use std::borrow::Borrow;
use std::cell::{Ref, RefCell, RefMut};
use std::hash::Hash;
use std::ops::Deref;
use std::rc::Rc;
use enum_dispatch::enum_dispatch;

// Note that we clone the map values when caching values, but callers usually use Rcs or types that
// wrap Rcs, so this is cheap.
//
// PORT_NOTE: Known as "MapWithCachedArrays" in the JS code. Arrays mean something different in
// Rust than JS, so I've renamed this to "CachedLinkedHashMap", to indicate the iterators are
// cached ("LinkedHashMapWithCachedIterators" seemed too long).
#[derive(Debug)]
pub(crate) struct CachedLinkedHashMap<V: Clone> {
    // PORT_NOTE: JS's Map maintains actual insertion order. Rust's HashMap makes no guarantees
    // about order, while the indexmap crate maintains order as long as all you do is inserts (their
    // remove() implementation breaks ordering, since it swaps the last inserted element in place of
    // the removed one). The hashlink crate gives actual insertion order even with removals, which
    // is why we use it here.
    map: LinkedHashMap<Rc<str>, V>,
    cached_keys: RefCell<Option<Rc<[Rc<str>]>>>,
    cached_values: RefCell<Option<Rc<[V]>>>,
}

// PORT_NOTE: The method names of JS maps differ from those of Rust maps (e.g. JS's "set" is Rust's
// "insert", JS's "delete" is Rust's "remove"), so we translate to Rust names where appropriate.
impl<V: Clone> CachedLinkedHashMap<V> {
    pub(crate) fn new() -> Self {
        Self {
            map: LinkedHashMap::new(),
            cached_keys: RefCell::new(None),
            cached_values: RefCell::new(None),
        }
    }

    pub(crate) fn cached_keys(&self) -> Rc<[Rc<str>]> {
        Rc::clone(self.cached_keys.borrow_mut().get_or_insert_with(|| {
            Rc::from(
                self.map
                    .keys()
                    .map(|key| Rc::clone(key))
                    .collect::<Vec<Rc<str>>>(),
            )
        }))
    }

    pub(crate) fn cached_values(&self) -> Rc<[V]> {
        Rc::clone(self.cached_values.borrow_mut().get_or_insert_with(|| {
            Rc::from(self.map.values().map(|val| val.clone()).collect::<Vec<V>>())
        }))
    }

    pub(crate) fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.clear_cached_values();
        self.map.get_mut(key)
    }

    // PORT_NOTE: You might be wondering why we delegate to replace() instead of insert() here. The
    // reason is JS Maps don't change an item's position in "insertion order" when a duplicate is
    // inserted, which is closer to the behavior of replace() rather than insert().
    /// If the key already exists, this will update its value while maintaining its current position
    /// in the list, and return the old value. If not, it adds the entry to the back of the list and
    /// returns None.
    pub(crate) fn replace(&mut self, key: Rc<str>, val: V) -> Option<V> {
        let prev = self.map.replace(key, val);
        if prev.is_some() {
            self.clear_cached_values();
        } else {
            self.clear_cache();
        }
        prev
    }

    /// If the key exists, this will remove it and return its value. Otherwise, returns None.
    pub(crate) fn remove(&mut self, key: &str) -> Option<V> {
        let res = self.map.remove(key);
        if res.is_some() {
            self.clear_cache();
        }
        res
    }

    fn clear_cache(&self) {
        self.clear_cached_keys();
        self.clear_cached_values();
    }

    fn clear_cached_keys(&self) {
        *self.cached_keys.borrow_mut() = None;
    }

    fn clear_cached_values(&self) {
        *self.cached_values.borrow_mut() = None;
    }
}

impl<V: Clone> Deref for CachedLinkedHashMap<V> {
    type Target = LinkedHashMap<Rc<str>, V>;

    // PORT_NOTE: The JS code has no concept of delegation, so it tried to manually delegate a few
    // methods of Map. In Rust, there's a pattern to implement delegation via Deref with new types,
    // so we do that here to delegate specifically immutable methods.
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

// Note that we clone the map values when caching values, but callers usually use Rcs or types that
// wrap Rcs, so this is cheap.
#[derive(Debug)]
pub(crate) struct CachedLinkedHashSet<V: Eq + Hash + Clone> {
    // PORT_NOTE: JS's Set maintains actual insertion order. Rust's HashSet makes no guarantees
    // about order, while the indexmap crate maintains order as long as all you do is inserts (their
    // remove() implementation breaks ordering, since it swaps the last inserted element in place of
    // the removed one). The hashlink crate gives actual insertion order even with removals, which
    // is why we use it here.
    set: LinkedHashSet<V>,
    cached_values: RefCell<Option<Rc<[V]>>>,
}

impl<V: Eq + Hash + Clone> CachedLinkedHashSet<V> {
    pub(crate) fn new() -> Self {
        Self {
            set: LinkedHashSet::new(),
            cached_values: RefCell::new(None),
        }
    }

    pub(crate) fn cached_values(&self) -> Rc<[V]> {
        Rc::clone(self.cached_values.borrow_mut().get_or_insert_with(|| {
            Rc::from(self.set.iter().map(|val| val.clone()).collect::<Vec<V>>())
        }))
    }

    // PORT_NOTE: You might be wondering why we delegate to replace() instead of insert() here. The
    // reason is JS Sets don't change an item's position in "insertion order" when a duplicate is
    // inserted, which is closer to the behavior of replace() rather than insert().
    /// If the value already exists, this method does nothing and returns false. If not, it adds
    /// the value to the back of the list and returns true.
    pub(crate) fn replace(&mut self, val: V) -> bool {
        // Note that the behavior for LinkedHashSet.replace() is to actually replace the value, even
        // if they're equal. We don't want to replace the value when equal, as it would mean
        // regenerating the cache, so we do a check here.
        if self.set.contains(&val) {
            false
        } else {
            self.set.replace(val);
            self.clear_cached_values();
            true
        }
    }

    /// If the value already exists, this method does nothing and returns false. If not, it adds
    /// the value to the front of the list and returns true.
    pub(crate) fn replace_front(&mut self, val: V) -> bool {
        // Note that the behavior for LinkedHashSet.replace() is to actually replace the value, even
        // if they're equal. We don't want to replace the value when equal, as it would mean
        // regenerating the cache, so we do a check here.
        if self.set.contains(&val) {
            false
        } else {
            self.set.replace(val.clone());
            self.set.to_front(&val);
            self.clear_cached_values();
            true
        }
    }

    /// If the value exists, this will remove it and return true. Otherwise, returns false.
    pub(crate) fn remove<Q>(&mut self, val: &Q) -> bool
    where
        V: Borrow<Q>,
        Q: ?Sized + Eq + Hash,
    {
        let is_modified = self.set.remove(val);
        if is_modified {
            self.clear_cached_values();
            true
        } else {
            false
        }
    }

    fn clear_cached_values(&self) {
        *self.cached_values.borrow_mut() = None;
    }
}

impl<V: Eq + Hash + Clone> Deref for CachedLinkedHashSet<V> {
    type Target = LinkedHashSet<V>;

    // PORT_NOTE: The JS code has no concept of delegation, so it tried to manually delegate a few
    // methods of Map. In Rust, there's a pattern to implement delegation via Deref with new types,
    // so we do that here to delegate specifically immutable methods.
    fn deref(&self) -> &Self::Target {
        &self.set
    }
}

// IndexMap makes iterating through keys and values fast, but it has the caveat that it doesn't
// maintain iteration order if you remove elements. This class wraps IndexMap to avoid exposing
// remove(), which is fine in the cases where removals happen around clones.
#[derive(Debug, Clone)]
pub struct InsertOnlyIndexMap<V> {
    map: IndexMap<Rc<str>, V>,
}

impl<V> InsertOnlyIndexMap<V> {
    pub fn new() -> InsertOnlyIndexMap<V> {
        InsertOnlyIndexMap {
            map: IndexMap::new(),
        }
    }

    pub fn insert(&mut self, key: Rc<str>, val: V) -> Option<V> {
        self.map.insert(key, val)
    }
}

impl<V> Deref for InsertOnlyIndexMap<V> {
    type Target = IndexMap<Rc<str>, V>;

    // Only delegate immutable methods to the underlying map.
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

// When using borrow() and borrow_mut() with RefCell, it's easy to forget to call drop() in the
// middle of a function (e.g. you call borrow(), then call a function that mutates but you miss the
// drop() in between). This is partly because it's easy to forget what's been borrowed. The below
// idiom with closures makes it clearer when we're using borrows and when they fall out of scope.
pub(crate) trait WithBorrow<T> {
    fn with_borrow<R, F: FnOnce(Ref<T>) -> R>(&self, f: F) -> R;
    fn with_borrow_mut<R, F: FnOnce(RefMut<T>) -> R>(&self, f: F) -> R;
}

impl<T> WithBorrow<T> for RefCell<T> {
    fn with_borrow<R, F: FnOnce(Ref<T>) -> R>(&self, f: F) -> R {
        f(self.borrow())
    }

    fn with_borrow_mut<R, F: FnOnce(RefMut<T>) -> R>(&self, f: F) -> R {
        f(self.borrow_mut())
    }
}
