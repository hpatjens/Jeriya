use crate::{elements::rigid_mesh, TransactionProcessor};

#[derive(Debug, Clone)]
pub enum Event {
    RigidMesh(rigid_mesh::Event),
}

pub struct TransactionRecorder<'t, T: TransactionProcessor> {
    // `transaction` is an Option because we want to be able to take it in `drop` instead of cloning it
    transaction: Option<Transaction>,
    transaction_processor: &'t T,
}

impl<T: TransactionProcessor> TransactionRecorder<'_, T> {
    /// Pushes an event to the transaction
    pub fn push(&mut self, event: Event) {
        self.transaction.as_mut().expect("no transaction").push(event);
    }

    /// Finishes the recording of the transaction. The transaction is sent to the [`TransactionProcessor`].
    ///
    /// Calling `TransactionRecorder::finish` has the same effect as dropping the `TransactionRecorder`.
    pub fn finish(self) {
        drop(self);
    }
}

impl<T: TransactionProcessor> Drop for TransactionRecorder<'_, T> {
    fn drop(&mut self) {
        self.transaction_processor.process(self.transaction.take().expect("no transaction"));
    }
}

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

    /// Starts the recording of a [`Transaction`]. The [`Transaction`] is sent to the [`TransactionProcessor`] when it is dropped.
    pub fn record<'t, T: TransactionProcessor>(transaction_processor: &'t T) -> TransactionRecorder<T> {
        TransactionRecorder {
            transaction_processor,
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

    /// Returns the number of events in the transaction
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns whether the transaction is considered processed
    pub fn is_considered_processed(&self) -> bool {
        self.is_considered_processed
    }

    /// Sets whether the transaction is considered processed
    pub fn set_is_considered_processed(&mut self, is_considered_processed: bool) {
        self.is_considered_processed = is_considered_processed;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record() {
        struct TransactionRecorder;
        impl TransactionProcessor for TransactionRecorder {
            fn process(&self, mut transaction: Transaction) {
                transaction.set_is_considered_processed(true);
                assert_eq!(transaction.len(), 1);
                assert_eq!(transaction.iter().count(), 1);
                for _event in &transaction {}
                let event = transaction.iter().next().unwrap();
                assert!(matches!(event, Event::RigidMesh(rigid_mesh::Event::Noop)));
            }
        }
        let transaction_recorder = TransactionRecorder;
        let mut transaction_recorder = Transaction::record(&transaction_recorder);
        transaction_recorder.push(Event::RigidMesh(rigid_mesh::Event::Noop));
        drop(transaction_recorder);
    }

    #[test]
    #[should_panic]
    fn drop_transaction() {
        let transaction = Transaction::new();
        drop(transaction);
    }
}
