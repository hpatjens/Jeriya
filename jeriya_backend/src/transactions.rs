use std::{mem, sync::Arc};

use crate::elements::rigid_mesh;

/// Trait that enables sending [`Transaction`]s to the renderer
pub trait TransactionProcessor {
    fn process(&self, transaction: Transaction);
}

/// Trait that is implemented by the [`Renderer`] to provide a [`TransactionProcessor`] implementation.
pub trait IntoTransactionProcessor<'s> {
    type TransactionProcessor: TransactionProcessor + 's;
    fn into_transaction_processor(&'s self) -> &'s Arc<Self::TransactionProcessor>;
}

/// Trait that is implemented by types that store [`Event`]s.
pub trait PushEvent {
    fn push_event(&mut self, event: Event);
}

/// An event that is sent to the renderer to be processed as part of a [`Transaction`].
#[derive(Debug, Clone)]
pub enum Event {
    RigidMesh(rigid_mesh::Event),
}

pub struct TransactionRecorder<'t, T: TransactionProcessor> {
    // `transaction` is an Option because we want to be able to take it in `drop` instead of cloning it
    transaction: Option<Transaction>,
    transaction_processor: &'t T,
}

impl<T: TransactionProcessor> PushEvent for TransactionRecorder<'_, T> {
    fn push_event(&mut self, event: Event) {
        self.push(event);
    }
}

impl<T: TransactionProcessor> TransactionRecorder<'_, T> {
    /// Pushes an event to the [`Transaction`].
    ///
    /// # Example
    ///
    /// ```
    /// use jeriya_backend::{
    ///     elements::rigid_mesh,
    ///     transactions::{Event, Transaction, TransactionProcessor}
    /// };
    /// # use jeriya_backend::transactions::MockRenderer;
    /// # let renderer = MockRenderer::new();
    /// let mut transaction_recorder = Transaction::record(&renderer);
    /// transaction_recorder.push(Event::RigidMesh(rigid_mesh::Event::Noop));
    /// transaction_recorder.finish();
    /// ```
    pub fn push(&mut self, event: Event) {
        self.transaction.as_mut().expect("no transaction").push(event);
    }

    /// Finishes the recording of the transaction. The transaction is sent to the [`TransactionProcessor`].
    ///
    /// Calling `TransactionRecorder::finish` has the same effect as dropping the `TransactionRecorder` but
    /// makes the intention and ordering of transactions clearer.
    ///
    /// # Example
    ///
    /// ```
    /// use jeriya_backend::{
    ///     elements::rigid_mesh,
    ///     transactions::{Event, Transaction, TransactionProcessor}
    /// };
    /// # use jeriya_backend::transactions::MockRenderer;
    /// # let renderer = MockRenderer::new();
    /// let mut transaction_recorder = Transaction::record(&renderer);
    /// transaction_recorder.push(Event::RigidMesh(rigid_mesh::Event::Noop));
    /// transaction_recorder.finish();
    /// ```
    pub fn finish(self) {
        drop(self);
    }
}

impl<T: TransactionProcessor> Drop for TransactionRecorder<'_, T> {
    fn drop(&mut self) {
        self.transaction_processor.process(self.transaction.take().expect("no transaction"));
    }
}

/// A set of instructions that are sent to the renderer to be processed in the next frame as one non-interuptable unit.
///
/// `Transaction`s are used to record changes to the *elements* and *instances* which have to be in a consistent state
/// when they are processed by the renderer. Changes to the *resources* are made asynchronously and are **not** recorded in
/// `Transaction`s. To create a `Transaction` use the [`Transaction::record`] method which returns a [`TransactionRecorder`].
/// Dropping or calling the [`TransactionRecorder::finish`] method on the `TransactionRecorder` will send the `Transaction`
/// to the renderer. If the ergonomics of the `TransactionRecorder` are not sufficient for the use case, a `Transaction`
/// can be created with the [`Transaction::new`] method. In this case the `Transaction` has to be sent to the renderer manually.
#[derive(Clone)]
pub struct Transaction {
    is_considered_processed: bool,
    events: Vec<Event>,
}

impl Transaction {
    /// Creates a new [`Transaction`]
    ///
    /// Consider using the [`Transaction::record`] method instead. A [`Transaction`] that is created with this method has to be sent to the renderer for processing manually. Otherwise it will panic.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            is_considered_processed: false,
        }
    }

    /// Starts the recording of a [`Transaction`].
    ///
    /// The [`Transaction`] is sent to the [`TransactionProcessor`] when the [`TransactionRecorder`]
    /// is dropped or the [`TransactionRecorder::finish`] method is called.
    ///
    /// # Example
    ///
    /// ```
    /// use jeriya_backend::{
    ///     elements::rigid_mesh,
    ///     transactions::{Event, Transaction, TransactionProcessor}
    /// };
    /// # use jeriya_backend::transactions::MockRenderer;
    /// # let renderer = MockRenderer::new();
    /// let mut transaction_recorder = Transaction::record(&renderer);
    /// transaction_recorder.push(Event::RigidMesh(rigid_mesh::Event::Noop));
    /// transaction_recorder.finish();
    /// ```
    pub fn record<'t, T: IntoTransactionProcessor<'t>>(renderer: &'t Arc<T>) -> TransactionRecorder<'t, T::TransactionProcessor> {
        TransactionRecorder {
            transaction_processor: renderer.into_transaction_processor(),
            transaction: Some(Self::new()),
        }
    }

    /// Pushes an event to the transaction
    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Returns an iterator over the events in the transaction
    pub fn iter(&self) -> impl Iterator<Item = &Event> {
        self.events.iter()
    }

    /// Returns the [`Event`]s from the `Transaction` for processing.
    pub fn process(mut self) -> Vec<Event> {
        self.set_is_processed(true);
        mem::take(&mut self.events)
    }

    /// Returns the number of events in the transaction
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns whether the transaction is considered processed
    pub fn is_processed(&self) -> bool {
        self.is_considered_processed
    }

    /// Sets whether the transaction is considered processed
    pub fn set_is_processed(&mut self, is_considered_processed: bool) {
        self.is_considered_processed = is_considered_processed;
    }
}

impl PushEvent for Transaction {
    fn push_event(&mut self, event: Event) {
        self.push(event);
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        assert!(
            self.is_considered_processed,
            "Transaction was not processed. Either use the Transaction::record method to create a TransactionRecorder or call Renderer::process and pass this Transaction."
        );
    }
}

impl<'a> IntoIterator for &'a Transaction {
    type Item = &'a Event;
    type IntoIter = std::slice::Iter<'a, Event>;

    fn into_iter(self) -> Self::IntoIter {
        self.events.iter()
    }
}

/// A [`TransactionProcessor`] that does nothing but set the transaction to `processed` before dropping it.
pub struct MockTransactionRecorder;

impl TransactionProcessor for MockTransactionRecorder {
    fn process(&self, mut transaction: Transaction) {
        // Otherwise the transaction will panic when dropped
        transaction.set_is_processed(true);
        drop(transaction);
    }
}

/// A mock that acts as the `Renderer` in the context of [`Transaction`]s.
pub struct MockRenderer(Arc<MockTransactionRecorder>);

impl MockRenderer {
    /// Creates a new `MockRenderer`
    pub fn new() -> Arc<MockRenderer> {
        Arc::new(Self(Arc::new(MockTransactionRecorder)))
    }
}

impl<'s> IntoTransactionProcessor<'s> for MockRenderer {
    type TransactionProcessor = MockTransactionRecorder;
    fn into_transaction_processor(&self) -> &Arc<Self::TransactionProcessor> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn record() {
        struct TransactionRecorder;
        impl TransactionProcessor for TransactionRecorder {
            fn process(&self, mut transaction: Transaction) {
                transaction.set_is_processed(true);
                assert_eq!(transaction.len(), 1);
                assert_eq!(transaction.iter().count(), 1);
                for _event in &transaction {}
                let event = transaction.iter().next().unwrap();
                assert!(matches!(event, Event::RigidMesh(rigid_mesh::Event::Noop)));
            }
        }
        struct DummyRenderer(Arc<TransactionRecorder>);
        impl<'s> IntoTransactionProcessor<'s> for DummyRenderer {
            type TransactionProcessor = TransactionRecorder;
            fn into_transaction_processor(&'s self) -> &'s Arc<Self::TransactionProcessor> {
                &self.0
            }
        }
        let renderer = Arc::new(DummyRenderer(Arc::new(TransactionRecorder)));
        let mut transaction_recorder = Transaction::record(&renderer);
        transaction_recorder.push(Event::RigidMesh(rigid_mesh::Event::Noop));
        drop(transaction_recorder);
    }

    #[test]
    fn process() {
        let mut transaction = Transaction::new();
        transaction.push(Event::RigidMesh(rigid_mesh::Event::Noop));
        for event in transaction.process() {
            assert!(matches!(event, Event::RigidMesh(rigid_mesh::Event::Noop)));
        }
    }

    #[test]
    #[should_panic]
    fn drop_transaction() {
        let transaction = Transaction::new();
        drop(transaction);
    }
}
