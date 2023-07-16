use std::{collections::VecDeque, fmt, hash::Hasher, marker::PhantomData, mem};

#[derive(Copy, PartialOrd, Ord)]
pub struct Handle<T> {
    index: usize,
    generation: usize,
    phantom_data: PhantomData<T>,
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handle")
            .field("index", &self.index)
            .field("generation", &self.generation)
            .finish()
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Handle<T> {}

impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
        self.phantom_data.hash(state);
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            generation: self.generation,
            phantom_data: self.phantom_data,
        }
    }
}

impl<T> Handle<T> {
    fn new(index: usize, generation: usize) -> Self {
        Self {
            index,
            generation,
            phantom_data: PhantomData,
        }
    }

    /// Returns the index of the handle.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Returns the generation of the handle.
    pub fn generation(&self) -> usize {
        self.generation
    }
}

pub struct IndexingContainer<T> {
    data: Vec<T>,
    generations: Vec<usize>,
    free_list: VecDeque<usize>,
}

impl<T> Default for IndexingContainer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default> IndexingContainer<T> {
    /// Removes the element at the given handle and returns it.
    ///
    /// # Notes
    ///
    /// This is currently only implemented for `T: Default` to prevent unsafe code.
    pub fn remove(&mut self, handle: &Handle<T>) -> Option<T> {
        if handle.generation() == self.generations[handle.index()] {
            self.generations[handle.index()] += 1;
            self.free_list.push_back(handle.index());
            Some(mem::take(&mut self.data[handle.index()]))
        } else {
            None
        }
    }
}

impl<T> IndexingContainer<T> {
    /// Creates a new empty container.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            generations: Vec::new(),
            free_list: VecDeque::new(),
        }
    }

    /// Inserts a new element into the container.
    pub fn insert(&mut self, value: T) -> Handle<T> {
        if let Some(free_index) = self.free_list.pop_front() {
            self.data[free_index] = value;
            Handle::new(free_index, self.generations[free_index])
        } else {
            let index = self.data.len();
            self.data.push(value);
            self.generations.push(0);
            Handle::new(index, 0)
        }
    }

    /// Returns a reference to the element at the given handle.
    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        if handle.generation() == self.generations[handle.index()] {
            Some(&self.data[handle.index()])
        } else {
            None
        }
    }

    /// Returns a mutable reference to the element at the given handle.
    pub fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        if handle.generation() == self.generations[handle.index()] {
            Some(&mut self.data[handle.index()])
        } else {
            None
        }
    }

    /// Returns the number of elements in the container.
    pub fn len(&self) -> usize {
        self.data.len() - self.free_list.len()
    }

    /// Returns `true` if the container contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of elements the container can hold without reallocating.
    pub fn free_count(&self) -> usize {
        self.free_list.len()
    }

    /// Returns a slice containing all elements in the container.
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use super::*;

    #[test]
    fn test_insert() {
        let mut container = IndexingContainer::<usize>::new();
        assert_eq!(container.len(), 0);
        assert_eq!(container.free_count(), 0);

        let handle = container.insert(0);
        assert_eq!(handle.index(), 0);
        assert_eq!(handle.generation(), 0);
        assert_eq!(container.len(), 1);
        assert_eq!(container.free_count(), 0);
    }

    #[test]
    fn test_remove() {
        let mut container = IndexingContainer::<usize>::new();
        let handle = container.insert(7);

        let value1 = container.remove(&handle);
        assert_eq!(value1, Some(7));
        assert_eq!(container.len(), 0);
        assert_eq!(container.free_count(), 1);

        let value2 = container.remove(&handle);
        assert_eq!(value2, None);
        assert_eq!(container.len(), 0);
        assert_eq!(container.free_count(), 1);
    }

    #[test]
    fn test_get() {
        let mut container = IndexingContainer::<usize>::new();
        let handle = container.insert(7);
        let value = container.get(&handle).unwrap();
        assert_eq!(value, &7);
    }

    #[test]
    fn test_get_mut() {
        let mut container = IndexingContainer::<usize>::new();
        let handle = container.insert(7);
        let value = container.get_mut(&handle).unwrap();
        *value += 1;
        assert_eq!(value, &mut 8);
    }

    #[test]
    fn test_reinsert() {
        let mut container = IndexingContainer::<usize>::new();

        let handle1 = container.insert(7);
        assert_eq!(handle1.index(), 0);
        assert_eq!(handle1.generation(), 0);

        let value1 = container.remove(&handle1).unwrap();
        assert_eq!(value1, 7);

        let handle2 = container.insert(8);
        assert_eq!(handle2.generation(), 1);

        let value2 = container.remove(&handle2).unwrap();
        assert_eq!(value2, 8);

        assert_eq!(container.free_count(), 1);
        assert_eq!(container.len(), 0);
    }

    #[test]
    fn test_drop() {
        struct Test(Arc<AtomicUsize>);
        impl Drop for Test {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let counter = Arc::new(AtomicUsize::new(0));

        let mut container = IndexingContainer::<Test>::new();
        container.insert(Test(counter.clone()));
        container.insert(Test(counter.clone()));
        container.insert(Test(counter.clone()));
        drop(container);

        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_as_slice() {
        let mut container = IndexingContainer::<usize>::new();
        let handle = container.insert(7);
        container.insert(8);
        container.insert(9);
        assert_eq!(container.as_slice(), &[7, 8, 9]);

        // The removed element is set to the default value.
        container.remove(&handle);
        assert_eq!(container.as_slice(), &[0, 8, 9]);
    }
}
